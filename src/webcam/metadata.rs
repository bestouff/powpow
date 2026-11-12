//! EXIF for archived captures, without recompressing their image data.

use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use img_parts::Bytes;
use little_exif::{
    exif_tag::ExifTag, filetype::FileExtension, ifd::ExifTagGroup, metadata::Metadata,
    rational::uR64,
};

use super::config::Coordinates;

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;

#[derive(Clone, Default)]
pub struct Settings {
    coordinates: Option<Coordinates>,
    author: String,
}

impl Settings {
    pub fn new(coordinates: Option<Coordinates>, name: &str, email: &str) -> Self {
        let author = match (name.trim(), email.trim()) {
            (name, "") => name.into(),
            ("", email) => email.into(),
            (name, email) => format!("{name} <{email}>"),
        };
        Self {
            coordinates,
            author,
        }
    }

    pub fn encode(&self, existing: Option<Bytes>, received_at: DateTime<Utc>) -> Result<Vec<u8>> {
        let mut metadata = match existing {
            Some(bytes) => match Metadata::new_from_vec(&bytes.to_vec(), FileExtension::TIFF) {
                Ok(metadata) => metadata,
                Err(error) => {
                    tracing::warn!(%error, "Replacing invalid webcam EXIF");
                    Metadata::new()
                }
            },
            None => Metadata::new(),
        };
        let date = received_at.format("%Y:%m:%d %H:%M:%S").to_string();
        let subseconds = format!("{:09}", received_at.timestamp_subsec_nanos());
        for tag in [
            ExifTag::ModifyDate(date.clone()),
            ExifTag::DateTimeOriginal(date.clone()),
            ExifTag::CreateDate(date),
            ExifTag::OffsetTime("+00:00".into()),
            ExifTag::OffsetTimeOriginal("+00:00".into()),
            ExifTag::OffsetTimeDigitized("+00:00".into()),
            ExifTag::SubSecTime(subseconds.clone()),
            ExifTag::SubSecTimeOriginal(subseconds.clone()),
            ExifTag::SubSecTimeDigitized(subseconds),
            ExifTag::Artist(self.author.clone()),
        ] {
            metadata.set_tag(tag);
        }
        // XPAuthor also preserves accented names in readers that expect ASCII in Artist.
        let unicode_author = self
            .author
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect();
        metadata.set_tag(ExifTag::UnknownINT8U(
            unicode_author,
            0x9c9d,
            ExifTagGroup::GENERIC,
        ));
        if let Some(coordinates) = self.coordinates {
            for tag in [
                ExifTag::GPSVersionID(vec![2, 3, 0, 0]),
                ExifTag::GPSMapDatum("WGS-84".into()),
                ExifTag::GPSLatitudeRef(if coordinates.latitude < 0.0 { "S" } else { "N" }.into()),
                ExifTag::GPSLatitude(degrees_minutes_seconds(coordinates.latitude)),
                ExifTag::GPSLongitudeRef(
                    if coordinates.longitude < 0.0 {
                        "W"
                    } else {
                        "E"
                    }
                    .into(),
                ),
                ExifTag::GPSLongitude(degrees_minutes_seconds(coordinates.longitude)),
            ] {
                metadata.set_tag(tag);
            }
        }
        let exif = metadata.encode().context("Cannot encode webcam EXIF")?;
        // Every capture also has JPEG previews: APP1 includes a two-byte length and Exif\0\0.
        ensure!(
            exif.len() <= 65_527,
            "Webcam EXIF exceeds the JPEG metadata limit"
        );
        Ok(exif)
    }
}

fn degrees_minutes_seconds(coordinate: f64) -> Vec<uR64> {
    let coordinate = coordinate.abs();
    let minutes = coordinate.fract() * 60.0;
    vec![
        coordinate.floor().into(),
        minutes.floor().into(),
        (minutes.fract() * 60.0).into(),
    ]
}
