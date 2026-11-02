//! Filesystem webcam archive. Only complete, decoded images enter the public index.

pub mod config;
pub mod ftps;
#[cfg(test)]
mod tests;

use std::{
    collections::BTreeSet,
    io::{Cursor, Write},
    ops::Bound::{Excluded, Unbounded},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use anyhow::{Context, Result, ensure};
use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use tokio::sync::Semaphore;

pub const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
pub const PAGE_SIZE: usize = 48;

#[derive(Clone, Debug, serde::Serialize)]
pub struct Frame {
    pub filename: String,
    pub received_at: DateTime<Utc>,
    pub username: String,
}

impl Frame {
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
    pub page: usize,
    pub pages: usize,
    pub previous: Option<Frame>,
    pub next: Option<Frame>,
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
        if create {
            tokio::fs::create_dir_all(&store.directory)
                .await
                .context("Cannot create WEBCAM_DIRECTORY")?;
        }
        let mut entries = match tokio::fs::read_dir(&store.directory).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(store),
            Err(error) => return Err(error).context("Cannot read WEBCAM_DIRECTORY"),
        };
        let mut images = BTreeSet::new();
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type().await?.is_file() && Frame::parse(&name).is_some() {
                images.insert(name);
            }
        }
        tracing::info!(images = images.len(), directory = %store.directory.display(), "Webcam archive loaded");
        *store.images.write().expect("webcam index lock") = images;
        Ok(store)
    }

    pub fn latest(&self) -> Option<Frame> {
        self.images
            .read()
            .expect("webcam index lock")
            .last()
            .and_then(|name| Frame::parse(name))
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

    pub fn gallery(&self, date: Option<NaiveDate>, filename: Option<&str>, page: usize) -> Gallery {
        let images = self.images.read().expect("webcam index lock");
        let requested = filename
            .filter(|name| images.contains(*name))
            .and_then(Frame::parse);
        let date = requested
            .as_ref()
            .map(|frame| frame.received_at.date_naive())
            .or(date)
            .or_else(|| {
                images
                    .last()
                    .and_then(|name| Frame::parse(name))
                    .map(|frame| frame.received_at.date_naive())
            })
            .unwrap_or_else(|| Utc::now().date_naive());
        let day_start = format!("{date}T");
        let day_end = format!("{date}U");
        let day: Vec<_> = images.range(day_start..day_end).rev().collect();
        let pages = day.len().div_ceil(PAGE_SIZE).max(1);
        let page = requested
            .as_ref()
            .and_then(|frame| day.iter().position(|name| **name == frame.filename))
            .map_or(page, |position| position / PAGE_SIZE)
            .min(pages - 1);
        let shown: Vec<_> = day
            .iter()
            .skip(page * PAGE_SIZE)
            .take(PAGE_SIZE)
            .filter_map(|name| Frame::parse(name))
            .collect();
        let selected = requested.or_else(|| shown.first().cloned());
        let (previous, next) = selected.as_ref().map_or((None, None), |frame| {
            let previous = images
                .range(..frame.filename.clone())
                .next_back()
                .and_then(|name| Frame::parse(name));
            let next = images
                .range((Excluded(frame.filename.clone()), Unbounded))
                .next()
                .and_then(|name| Frame::parse(name));
            (previous, next)
        });
        Gallery {
            selected,
            images: shown,
            date,
            page,
            pages,
            previous,
            next,
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
            validate_image(&bytes, &extension)?;
            store.publish(&bytes, &username, &extension)
        })
        .await?
    }

    fn publish(&self, bytes: &[u8], username: &str, extension: &str) -> Result<Frame> {
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
                    self.images
                        .write()
                        .expect("webcam index lock")
                        .insert(filename.clone());
                    tracing::info!(%filename, bytes = bytes.len(), "Webcam image archived");
                    return Ok(Frame {
                        filename,
                        received_at,
                        username: username.into(),
                    });
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

fn validate_image(bytes: &[u8], extension: &str) -> Result<()> {
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
    reader.decode().context("Invalid webcam image")?;
    Ok(())
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
