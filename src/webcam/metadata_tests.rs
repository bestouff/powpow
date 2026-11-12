use super::*;
use crate::webcam::{
    Store,
    tests::{assert_ascii, assert_timestamp},
};
use exif::{In, Tag, Value};
use img_parts::{DynImage, ImageEXIF};
use std::io::Cursor;

fn without_exif(bytes: Vec<u8>) -> Result<Vec<u8>> {
    let mut image = DynImage::from_bytes(bytes.into())?.unwrap();
    image.set_exif(None);
    let mut output = Vec::new();
    image.encoder().write_to(&mut output)?;
    Ok(output)
}

fn assert_position(exif: &exif::Exif, coordinates: Coordinates) {
    for (tag, reference, expected, positive, negative) in [
        (
            Tag::GPSLatitude,
            Tag::GPSLatitudeRef,
            coordinates.latitude,
            "N",
            "S",
        ),
        (
            Tag::GPSLongitude,
            Tag::GPSLongitudeRef,
            coordinates.longitude,
            "E",
            "W",
        ),
    ] {
        let Value::Rational(values) = &exif.get_field(tag, In::PRIMARY).unwrap().value else {
            panic!("Expected GPS rationals");
        };
        assert_eq!(values.len(), 3);
        let degrees = values[0].to_f64() + values[1].to_f64() / 60.0 + values[2].to_f64() / 3600.0;
        assert!(
            (degrees - expected.abs()).abs() < 1e-7,
            "{degrees} != {expected}"
        );
        let expected_reference = if expected < 0.0 { negative } else { positive };
        assert_ascii(exif, reference, expected_reference);
    }
}

#[tokio::test]
async fn exif_survives_every_image_format_and_preview_regeneration() -> Result<()> {
    let author = "École ⛷ <webcam@example.org>";
    for coordinates in [
        Coordinates {
            latitude: 45.31,
            longitude: 5.85,
        },
        Coordinates {
            latitude: -33.865_143,
            longitude: -151.2099,
        },
        Coordinates {
            latitude: -90.0,
            longitude: 180.0,
        },
        Coordinates {
            latitude: 0.0,
            longitude: 0.0,
        },
    ] {
        let directory = tempfile::tempdir()?;
        let settings = Settings::new(Some(coordinates), " École ⛷ ", " webcam@example.org ");
        let store = Store::open(directory.path().into(), true, settings).await?;
        for (format, extension) in [
            (image::ImageFormat::Jpeg, "jpg"),
            (image::ImageFormat::Png, "png"),
            (image::ImageFormat::WebP, "webp"),
        ] {
            let mut source = Cursor::new(Vec::new());
            let pixels = image::RgbImage::from_fn(16, 12, |x, y| {
                image::Rgb([
                    u8::try_from(x * 15).unwrap(),
                    u8::try_from(y * 20).unwrap(),
                    80,
                ])
            });
            image::DynamicImage::ImageRgb8(pixels).write_to(&mut source, format)?;
            let mut source = DynImage::from_bytes(source.into_inner().into())?.unwrap();
            let mut camera_exif = Metadata::new();
            camera_exif.set_tag(ExifTag::Make("Camera maker".into()));
            camera_exif.set_tag(ExifTag::Orientation(vec![1]));
            camera_exif.set_tag(ExifTag::Artist("Old author".into()));
            camera_exif.set_tag(ExifTag::DateTimeOriginal("2000:01:01 00:00:00".into()));
            camera_exif.set_tag(ExifTag::GPSLatitude(vec![
                1.0.into(),
                0.0.into(),
                0.0.into(),
            ]));
            source.set_exif(Some(camera_exif.encode()?.into()));
            let mut source_bytes = Vec::new();
            source.encoder().write_to(&mut source_bytes)?;
            let frame = store
                .save(source_bytes.clone(), "station".into(), extension.into())
                .await?;
            let archived = std::fs::read(store.path(&frame))?;
            assert_eq!(image::guess_format(&archived)?, format);
            assert_eq!(
                image::load_from_memory(&archived)?,
                image::load_from_memory(&source_bytes)?
            );
            assert_eq!(without_exif(archived.clone())?, without_exif(source_bytes)?);
            for path in [
                store.path(&frame),
                store.jpeg_path(&frame),
                directory.path().join(frame.thumbnail()),
            ] {
                let bytes = std::fs::read(&path)?;
                assert_timestamp(&bytes, &frame)?;
                let exif = exif::Reader::new().read_from_container(&mut Cursor::new(&bytes))?;
                assert_position(&exif, coordinates);
                assert_ascii(&exif, Tag::Artist, author);
                let unicode_author: Vec<u8> = author
                    .encode_utf16()
                    .chain([0])
                    .flat_map(u16::to_le_bytes)
                    .collect();
                let Value::Byte(actual_author) = &exif
                    .get_field(Tag(exif::Context::Tiff, 0x9c9d), In::PRIMARY)
                    .unwrap()
                    .value
                else {
                    panic!("Expected Unicode XPAuthor bytes");
                };
                assert_eq!(actual_author, &unicode_author);
                assert_ascii(&exif, Tag::Make, "Camera maker");
                assert_eq!(
                    exif.get_field(Tag::Orientation, In::PRIMARY)
                        .unwrap()
                        .value
                        .get_uint(0),
                    Some(1)
                );
                let expected_width = if path == directory.path().join(frame.thumbnail()) {
                    360
                } else {
                    16
                };
                assert_eq!(image::load_from_memory(&bytes)?.width(), expected_width);
            }
            let thumbnail = directory.path().join(frame.thumbnail());
            let original_thumbnail = std::fs::read(&thumbnail)?;
            std::fs::remove_file(&thumbnail)?;
            let original_jpeg = std::fs::read(store.jpeg_path(&frame))?;
            if extension != "jpg" {
                std::fs::remove_file(store.jpeg_path(&frame))?;
            }
            // Recovery copies the capture's EXIF, even if the current settings changed.
            Store::open(directory.path().into(), false, Settings::default()).await?;
            assert_eq!(std::fs::read(&thumbnail)?, original_thumbnail);
            assert_eq!(std::fs::read(store.jpeg_path(&frame))?, original_jpeg);
            assert_eq!(std::fs::read(store.path(&frame))?, archived);
        }
    }
    Ok(())
}

#[test]
fn collision_retry_uses_the_successful_filename_time() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::new(directory.path().into(), Settings::new(None, "Station", ""));
    let first: DateTime<Utc> = "2026-11-12T23:59:59.999999999Z".parse()?;
    let second = first + chrono::TimeDelta::nanoseconds(1);
    let existing = directory.path().join(format!(
        "{}_station.png",
        first.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
    ));
    let bytes = crate::webcam::tests::png();
    std::fs::write(&existing, &bytes)?;
    let mut times = [first, second].into_iter();
    let image = image::load_from_memory(&bytes)?;
    let frame = store.publish(bytes.clone(), "station", "png", &image, || {
        times.next().unwrap()
    })?;
    assert_eq!(frame.received_at, second);
    assert_eq!(std::fs::read(&existing)?, bytes);
    assert_timestamp(&std::fs::read(store.path(&frame))?, &frame)?;
    assert!(directory.path().read_dir()?.all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with('.')
    }));
    Ok(())
}

#[test]
fn missing_coordinates_and_partial_identity_still_produce_dated_exif() -> Result<()> {
    let time = "2026-11-12T12:34:56.000000007Z".parse()?;
    for (name, email, author) in [
        ("Station", "", "Station"),
        ("", "contact@example.org", "contact@example.org"),
        ("", "", ""),
    ] {
        let raw = Settings::new(None, name, email).encode(None, time)?;
        let exif = exif::Reader::new().read_raw(raw)?;
        assert!(exif.get_field(Tag::GPSLatitude, In::PRIMARY).is_none());
        assert!(exif.get_field(Tag::GPSLongitude, In::PRIMARY).is_none());
        assert_ascii(&exif, Tag::Artist, author);
        assert_ascii(&exif, Tag::SubSecTimeOriginal, "000000007");
    }
    Ok(())
}
