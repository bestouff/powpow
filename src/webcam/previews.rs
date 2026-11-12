//! Precomputed JPEG previews and per-camera, local-day manifests.

use std::{io::Write, path::Path};

use anyhow::Result;
use chrono::{Local, NaiveDate};
use image::{DynamicImage, codecs::jpeg::JpegEncoder};
use serde::Serialize;

use super::Frame;

#[derive(Serialize)]
struct Preview<'a> {
    filename: &'a str,
    thumbnail: String,
    received_at: chrono::DateTime<Local>,
    label: String,
}

#[derive(Serialize)]
struct DayList<'a> {
    date: NaiveDate,
    camera: &'a str,
    images: Vec<Preview<'a>>,
}

pub fn list_name(date: NaiveDate, camera: &str) -> String {
    format!("{date}_{camera}_list.json")
}

pub fn parse_list_name(filename: &str) -> Option<(NaiveDate, &str)> {
    let (date, camera) = filename.strip_suffix("_list.json")?.split_once('_')?;
    let date: NaiveDate = date.parse().ok()?;
    (super::config::valid_username(camera) && list_name(date, camera) == filename)
        .then_some((date, camera))
}

pub fn regular_file(path: &Path) -> bool {
    path.symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_file())
}

pub fn missing(directory: &Path, frame: &Frame) -> bool {
    !regular_file(&directory.join(frame.thumbnail()))
        || !regular_file(&directory.join(frame.jpeg()))
}

pub fn generate(directory: &Path, frame: &Frame, image: &DynamicImage) -> Result<()> {
    let thumbnail = directory.join(frame.thumbnail());
    if !regular_file(&thumbnail) {
        let small = image.thumbnail(480, 270).to_rgb8();
        write_atomic(&thumbnail, |file| {
            JpegEncoder::new_with_quality(file, 75).encode_image(&small)?;
            Ok(())
        })?;
    }
    // The stable .jpg endpoint also works for PNG/WebP cameras without transcoding requests.
    let jpeg = directory.join(frame.jpeg());
    if !regular_file(&jpeg) {
        write_atomic(&jpeg, |file| {
            JpegEncoder::new_with_quality(file, 90).encode_image(&image.to_rgb8())?;
            Ok(())
        })?;
    }
    Ok(())
}

pub fn write_list(directory: &Path, date: NaiveDate, camera: &str, frames: &[Frame]) -> Result<()> {
    let list = DayList {
        date,
        camera,
        images: frames
            .iter()
            .map(|frame| Preview {
                filename: &frame.filename,
                thumbnail: frame.thumbnail(),
                received_at: frame.received_at.with_timezone(&Local),
                label: frame.local_label(),
            })
            .collect(),
    };
    let bytes = serde_json::to_vec(&list)?;
    let path = directory.join(list_name(date, camera));
    if regular_file(&path) && std::fs::read(&path)? == bytes {
        return Ok(());
    }
    write_atomic(&path, |file| {
        file.write_all(&bytes)?;
        Ok(())
    })
}

fn write_atomic(path: &Path, write: impl FnOnce(&mut std::fs::File) -> Result<()>) -> Result<()> {
    let mut file = tempfile::Builder::new()
        .prefix(".preview-")
        .tempfile_in(path.parent().expect("archive file has a parent"))?;
    write(file.as_file_mut())?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}
