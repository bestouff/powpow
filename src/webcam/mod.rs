//! Filesystem webcam archive. Only complete, decoded images enter the public index.

pub mod config;
pub mod direct;
pub mod ftps;
pub mod previews;
#[cfg(test)]
mod tests;

use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Local, NaiveDate, SecondsFormat, TimeDelta, Utc};
use tokio::sync::Semaphore;

pub const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Frame {
    pub filename: String,
    pub received_at: DateTime<Utc>,
    pub username: String,
}

impl Frame {
    pub fn date(&self) -> NaiveDate {
        self.received_at.with_timezone(&Local).date_naive()
    }

    pub fn local_label(&self) -> String {
        self.received_at
            .with_timezone(&Local)
            .format("%d/%m/%Y à %H:%M:%S")
            .to_string()
    }

    pub fn thumbnail(&self) -> String {
        // Keep the original extension: derivatives cannot be mistaken for another camera.
        format!("{}_small.jpg", self.filename)
    }

    pub fn jpeg(&self) -> String {
        if self.mime_type() == "image/jpeg" {
            self.filename.clone()
        } else {
            format!("{}.jpg", self.filename)
        }
    }

    pub fn parse(filename: &str) -> Option<Self> {
        let (timestamp, rest) = filename.split_once('_')?;
        let (username, extension) = rest.rsplit_once('.')?;
        let received_at = DateTime::parse_from_rfc3339(timestamp)
            .ok()?
            .with_timezone(&Utc);
        if !config::valid_username(username)
            || image_format(extension).is_none()
            || timestamp != received_at.to_rfc3339_opts(SecondsFormat::Nanos, true)
        {
            return None;
        }
        Some(Self {
            filename: filename.into(),
            received_at,
            username: username.into(),
        })
    }

    pub fn mime_type(&self) -> &'static str {
        match self
            .filename
            .rsplit_once('.')
            .map(|(_, extension)| extension)
        {
            Some("png") => "image/png",
            Some("webp") => "image/webp",
            _ => "image/jpeg",
        }
    }
}

pub fn image_format(extension: &str) -> Option<image::ImageFormat> {
    match extension {
        "jpg" | "jpeg" => Some(image::ImageFormat::Jpeg),
        "png" => Some(image::ImageFormat::Png),
        "webp" => Some(image::ImageFormat::WebP),
        _ => None,
    }
}

pub struct Gallery {
    pub selected: Option<Frame>,
    pub images: Vec<Frame>,
    pub date: NaiveDate,
    pub camera: Option<String>,
    pub recent: [Option<Frame>; 3],
}

#[derive(Clone)]
pub struct Store {
    directory: PathBuf,
    images: Arc<RwLock<BTreeSet<String>>>,
    processors: Arc<Semaphore>,
}

impl Store {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            images: Arc::default(),
            processors: Arc::new(Semaphore::new(2)),
        }
    }

    pub async fn open(directory: PathBuf, create: bool) -> Result<Self> {
        let store = Self::new(directory);
        let loader = store.clone();
        tokio::task::spawn_blocking(move || loader.load(create)).await??;
        Ok(store)
    }

    fn load(&self, create: bool) -> Result<()> {
        if create {
            std::fs::create_dir_all(&self.directory).context("Cannot create WEBCAM_DIRECTORY")?;
        }
        let entries = match std::fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error).context("Cannot read WEBCAM_DIRECTORY"),
        };
        let mut images = BTreeSet::new();
        let mut days: BTreeMap<(NaiveDate, String), Vec<Frame>> = BTreeMap::new();
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !entry.file_type()?.is_file() {
                continue;
            }
            let Some(frame) = Frame::parse(&name) else {
                continue;
            };
            if previews::missing(&self.directory, &frame) {
                let mut bytes = Vec::new();
                std::fs::File::open(entry.path())?
                    .take(MAX_IMAGE_BYTES + 1)
                    .read_to_end(&mut bytes)?;
                let extension = name.rsplit_once('.').expect("frame extension").1;
                let image = match decode_image(&bytes, extension) {
                    Ok(image) => image,
                    Err(error) => {
                        tracing::warn!(%name, %error, "Skipping invalid webcam image");
                        continue;
                    }
                };
                previews::generate(&self.directory, &frame, &image)?;
            }
            days.entry((frame.date(), frame.username.clone()))
                .or_default()
                .push(frame);
            images.insert(name);
        }
        for ((date, camera), mut frames) in days {
            frames.sort_by(|a, b| a.filename.cmp(&b.filename));
            previews::write_list(&self.directory, date, &camera, &frames)?;
        }
        tracing::info!(images = images.len(), directory = %self.directory.display(), "Webcam archive and previews loaded");
        *self.images.write().expect("webcam index lock") = images;
        Ok(())
    }

    pub fn latest(&self) -> Option<Frame> {
        self.images
            .read()
            .expect("webcam index lock")
            .last()
            .and_then(|name| Frame::parse(name))
    }

    pub fn latest_for(&self, camera: &str) -> Option<Frame> {
        self.images
            .read()
            .expect("webcam index lock")
            .iter()
            .rev()
            .filter_map(|name| Frame::parse(name))
            .find(|frame| frame.username == camera)
    }

    pub fn get(&self, filename: &str) -> Option<Frame> {
        let frame = Frame::parse(filename)?;
        self.images
            .read()
            .expect("webcam index lock")
            .contains(filename)
            .then_some(frame)
    }

    pub fn path(&self, frame: &Frame) -> PathBuf {
        self.directory.join(&frame.filename)
    }

    pub fn thumbnail(&self, filename: &str) -> Option<PathBuf> {
        let frame = self.get(filename.strip_suffix("_small.jpg")?)?;
        Some(self.directory.join(frame.thumbnail()))
    }

    pub fn jpeg_path(&self, frame: &Frame) -> PathBuf {
        self.directory.join(frame.jpeg())
    }

    pub fn list_path(&self, filename: &str) -> Option<PathBuf> {
        let (date, camera) = previews::parse_list_name(filename)?;
        let images = self.images.read().expect("webcam index lock");
        (!day_frames(&images, date, camera).is_empty()).then(|| self.directory.join(filename))
    }

    pub fn gallery(
        &self,
        date: Option<NaiveDate>,
        filename: Option<&str>,
        camera: Option<&str>,
    ) -> Gallery {
        let images = self.images.read().expect("webcam index lock");
        let requested = filename
            .filter(|name| images.contains(*name))
            .and_then(Frame::parse);
        let camera = requested
            .as_ref()
            .map(|frame| frame.username.as_str())
            .or(camera)
            .map(str::to_owned)
            .or_else(|| {
                images
                    .last()
                    .and_then(|name| Frame::parse(name))
                    .map(|frame| frame.username)
            });
        let latest = images
            .iter()
            .rev()
            .filter_map(|name| Frame::parse(name))
            .find(|frame| Some(&frame.username) == camera.as_ref());
        let date = requested
            .as_ref()
            .map(Frame::date)
            .or(date)
            .or_else(|| latest.as_ref().map(Frame::date))
            .unwrap_or_else(|| Local::now().date_naive());
        let day = camera
            .as_ref()
            .map_or_else(Vec::new, |camera| day_frames(&images, date, camera));
        let selected = requested.or_else(|| day.last().cloned());
        let now = Utc::now();
        let recent = [1, 3, 5].map(|hours| {
            images
                .iter()
                .rev()
                .filter_map(|name| Frame::parse(name))
                .find(|frame| {
                    Some(&frame.username) == camera.as_ref()
                        && frame.received_at <= now - TimeDelta::hours(hours)
                })
        });
        Gallery {
            selected,
            images: day,
            date,
            camera,
            recent,
        }
    }

    pub async fn save(&self, bytes: Vec<u8>, username: String, extension: String) -> Result<Frame> {
        ensure!(
            config::valid_username(&username),
            "Invalid webcam account name"
        );
        ensure!(
            bytes.len() as u64 <= MAX_IMAGE_BYTES,
            "Webcam image exceeds 20 MiB"
        );
        let permit = self.processors.clone().acquire_owned().await?;
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let image = decode_image(&bytes, &extension)?;
            store.publish(&bytes, &username, &extension, &image)
        })
        .await?
    }

    fn publish(
        &self,
        bytes: &[u8],
        username: &str,
        extension: &str,
        image: &image::DynamicImage,
    ) -> Result<Frame> {
        let mut upload = tempfile::Builder::new()
            .prefix(".upload-")
            .tempfile_in(&self.directory)?;
        upload.write_all(bytes)?;
        upload.as_file().sync_all()?;
        // A fresh name is used even when the camera repeatedly uploads the same filename.
        // No-clobber publication also protects against clock collisions and clock corrections.
        for _ in 0..10 {
            let received_at = Utc::now();
            let filename = format!(
                "{}_{}.{}",
                received_at.to_rfc3339_opts(SecondsFormat::Nanos, true),
                username,
                extension
            );
            match upload.persist_noclobber(self.directory.join(&filename)) {
                Ok(_) => {
                    let frame = Frame {
                        filename,
                        received_at,
                        username: username.into(),
                    };
                    previews::generate(&self.directory, &frame, image)?;
                    // Publish the index and manifest together; concurrent uploads cannot lose entries.
                    let mut images = self.images.write().expect("webcam index lock");
                    let mut day = day_frames(&images, frame.date(), username);
                    day.push(frame.clone());
                    day.sort_by(|a, b| a.filename.cmp(&b.filename));
                    previews::write_list(&self.directory, frame.date(), username, &day)?;
                    images.insert(frame.filename.clone());
                    tracing::info!(filename = %frame.filename, bytes = bytes.len(), "Webcam image archived");
                    return Ok(frame);
                }
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                    upload = error.file;
                }
                Err(error) => return Err(error.error.into()),
            }
        }
        anyhow::bail!("Cannot allocate a unique webcam timestamp")
    }
}

fn day_frames(images: &BTreeSet<String>, date: NaiveDate, camera: &str) -> Vec<Frame> {
    // Local days can span neighboring UTC dates, including during DST changes.
    let start = format!("{}T", date.pred_opt().unwrap_or(date));
    let end = format!("{}U", date.succ_opt().unwrap_or(date));
    images
        .range(start..end)
        .filter_map(|name| Frame::parse(name))
        .filter(|frame| frame.username == camera && frame.date() == date)
        .collect()
}

fn decode_image(bytes: &[u8], extension: &str) -> Result<image::DynamicImage> {
    ensure!(
        bytes.len() as u64 <= MAX_IMAGE_BYTES,
        "Webcam image exceeds 20 MiB"
    );
    let expected = image_format(extension).context("Unsupported webcam image extension")?;
    ensure!(
        image::guess_format(bytes)? == expected,
        "Image content does not match its extension"
    );
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), expected);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().context("Invalid webcam image")
}

pub async fn open_image(path: &Path) -> std::io::Result<tokio::fs::File> {
    if !tokio::fs::symlink_metadata(path)
        .await?
        .file_type()
        .is_file()
    {
        return Err(std::io::ErrorKind::NotFound.into());
    }
    tokio::fs::File::open(path).await
}
