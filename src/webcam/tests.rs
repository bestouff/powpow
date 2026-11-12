use super::*;
use chrono::{TimeDelta, TimeZone};

pub(super) fn png() -> Vec<u8> {
    let mut data = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(4, 3)
        .write_to(&mut data, image::ImageFormat::Png)
        .unwrap();
    data.into_inner()
}

#[tokio::test]
async fn archive_publishes_complete_images_and_recovers_after_restart() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(directory.path().into(), true).await?;
    let (first, second) = tokio::join!(
        store.save(png(), "station_nord".into(), "png".into()),
        store.save(png(), "station_nord".into(), "png".into())
    );
    let first = first?;
    let second = second?;
    assert_ne!(first.filename, second.filename);
    assert_eq!(
        Frame::parse(&first.filename).unwrap().username,
        "station_nord"
    );
    assert_eq!(std::fs::read(store.path(&first))?, png());
    assert_eq!(std::fs::read(store.path(&second))?, png());
    for frame in [&first, &second] {
        let thumbnail = std::fs::read(directory.path().join(frame.thumbnail()))?;
        assert_eq!(image::guess_format(&thumbnail)?, image::ImageFormat::Jpeg);
        assert!(Frame::parse(&frame.thumbnail()).is_none());
        assert!(Frame::parse(&frame.jpeg()).is_none());
    }
    let list_path = directory
        .path()
        .join(previews::list_name(first.date(), "station_nord"));
    let list: serde_json::Value = serde_json::from_slice(&std::fs::read(&list_path)?)?;
    let listed = list["images"].as_array().unwrap();
    assert_eq!(listed.len(), 2);
    for frame in [&first, &second] {
        assert!(
            listed
                .iter()
                .any(|entry| entry["filename"] == frame.filename
                    && entry["thumbnail"] == frame.thumbnail())
        );
    }
    let file_count = directory.path().read_dir()?.count();
    assert!(
        store
            .save(png(), "../escape".into(), "png".into())
            .await
            .is_err()
    );
    assert!(
        store
            .save(png(), "station".into(), "jpg".into())
            .await
            .is_err()
    );
    assert!(
        store
            .save(b"not an image".to_vec(), "station".into(), "jpg".into())
            .await
            .is_err()
    );
    assert_eq!(directory.path().read_dir()?.count(), file_count);

    std::fs::write(directory.path().join(".upload-unfinished"), png())?;
    std::fs::write(directory.path().join("unrelated.png"), png())?;
    let restarted = Store::open(directory.path().into(), false).await?;
    assert_eq!(
        restarted.latest().unwrap().filename,
        store.latest().unwrap().filename
    );
    assert_eq!(restarted.gallery(None, None, None).images.len(), 2);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(list_path)?)?,
        list
    );
    assert!(restarted.get("../outside.png").is_none());
    assert!(restarted.get(".upload-unfinished").is_none());
    Ok(())
}

#[tokio::test]
async fn gallery_lists_the_entire_local_day_and_keeps_cameras_separate() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let midnight = Local
        .with_ymd_and_hms(2026, 11, 2, 0, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    for minute in 0..55 {
        let received_at = midnight - TimeDelta::hours(1) + TimeDelta::minutes(minute);
        std::fs::write(
            directory.path().join(format!(
                "{}_station.png",
                received_at.to_rfc3339_opts(SecondsFormat::Nanos, true)
            )),
            png(),
        )?;
    }
    let newest = format!(
        "{}_station.png",
        midnight.to_rfc3339_opts(SecondsFormat::Nanos, true)
    );
    std::fs::write(directory.path().join(&newest), png())?;
    std::fs::write(
        directory
            .path()
            .join(newest.replace("_station", "_station_small")),
        png(),
    )?;
    let store = Store::open(directory.path().into(), false).await?;
    let day = "2026-11-01".parse()?;
    let latest = store.gallery(None, None, Some("station"));
    assert_eq!(latest.selected.unwrap().filename, newest);
    assert_eq!(latest.images.len(), 1);
    let first = store.gallery(Some(day), None, Some("station"));
    assert_eq!(first.images.len(), 55);
    assert!(
        first
            .images
            .windows(2)
            .all(|pair| pair[0].received_at < pair[1].received_at)
    );
    let chosen = store.gallery(None, Some(&first.images[0].filename), Some("station_small"));
    assert_eq!(chosen.date, day);
    assert_eq!(chosen.camera.as_deref(), Some("station"));
    assert_eq!(
        chosen.selected.unwrap().received_at,
        midnight - TimeDelta::hours(1)
    );
    let list: serde_json::Value = serde_json::from_slice(&std::fs::read(
        directory.path().join(previews::list_name(day, "station")),
    )?)?;
    assert_eq!(list["images"].as_array().unwrap().len(), 55);
    let other = store.gallery(None, None, Some("station_small"));
    assert_eq!(other.images.len(), 1);
    assert_eq!(other.images[0].username, "station_small");
    assert!(
        store
            .gallery(Some("2026-10-30".parse()?), None, Some("station"))
            .selected
            .is_none()
    );
    Ok(())
}

#[tokio::test]
async fn gallery_keeps_complete_local_days_across_daylight_saving_changes() -> Result<()> {
    // Run with TZ=Europe/Paris to cover both the 23-hour and 25-hour days.
    for date in ["2026-03-29", "2026-10-25"] {
        let date: NaiveDate = date.parse()?;
        let start = Local
            .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
            .unwrap()
            .with_timezone(&Utc);
        let end = Local
            .from_local_datetime(&date.succ_opt().unwrap().and_hms_opt(0, 0, 0).unwrap())
            .unwrap()
            .with_timezone(&Utc);
        let directory = tempfile::tempdir()?;
        let step = TimeDelta::minutes(30);
        let mut received_at = start - step;
        let mut expected = Vec::new();
        while received_at <= end {
            let filename = format!(
                "{}_station.png",
                received_at.to_rfc3339_opts(SecondsFormat::Nanos, true)
            );
            std::fs::write(directory.path().join(&filename), png())?;
            if received_at >= start && received_at < end {
                expected.push(filename);
            }
            received_at += step;
        }
        let store = Store::open(directory.path().into(), false).await?;
        let gallery = store.gallery(Some(date), None, None);
        let actual: Vec<_> = gallery
            .images
            .iter()
            .map(|frame| frame.filename.clone())
            .collect();
        assert_eq!(actual, expected, "{date}");
        let earliest = store.gallery(None, expected.first().map(String::as_str), None);
        assert_eq!(earliest.date, date);
        assert_eq!(earliest.selected.unwrap().received_at, start);
        let list: serde_json::Value = serde_json::from_slice(&std::fs::read(
            directory.path().join(previews::list_name(date, "station")),
        )?)?;
        assert_eq!(list["images"].as_array().unwrap().len(), expected.len());
        assert_eq!(list["images"][0]["label"], gallery.images[0].local_label());
    }
    Ok(())
}

#[tokio::test]
async fn existing_archives_get_missing_previews_and_time_shortcuts() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let now = Utc::now();
    let mut frames = Vec::new();
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(1920, 1080).write_to(&mut bytes, image::ImageFormat::Jpeg)?;
    for minutes in [0, 61, 181, 301] {
        let filename = format!(
            "{}_tkecole.jpg",
            (now - TimeDelta::minutes(minutes)).to_rfc3339_opts(SecondsFormat::Nanos, true)
        );
        std::fs::write(directory.path().join(&filename), bytes.get_ref())?;
        frames.push(Frame::parse(&filename).unwrap());
    }
    std::fs::write(
        directory
            .path()
            .join("2026-01-01T00:00:00.000000000Z_broken.jpg"),
        b"broken",
    )?;
    let store = Store::open(directory.path().into(), false).await?;
    let gallery = store.gallery(None, None, Some("tkecole"));
    for (recent, expected) in gallery.recent.iter().zip(&frames[1..]) {
        assert_eq!(recent.as_ref().unwrap().filename, expected.filename);
    }
    let thumbnail = directory.path().join(frames[0].thumbnail());
    assert_eq!(image::image_dimensions(&thumbnail)?, (480, 270));
    assert!(store.latest_for("broken").is_none());
    std::fs::remove_file(&thumbnail)?;
    let restarted = Store::open(directory.path().into(), false).await?;
    assert_eq!(image::image_dimensions(&thumbnail)?, (480, 270));
    assert_eq!(restarted.latest().unwrap().filename, frames[0].filename);
    assert_eq!(std::fs::read(store.path(&frames[0]))?, *bytes.get_ref());
    Ok(())
}

#[test]
fn time_shortcuts_choose_the_nearest_capture_within_thirty_minutes_on_either_side() {
    let store = Store::new(PathBuf::new());
    let now = Utc::now();
    let filename = |minutes, camera| {
        format!(
            "{}_{}.jpg",
            (now - TimeDelta::minutes(minutes)).to_rfc3339_opts(SecondsFormat::Nanos, true),
            camera,
        )
    };
    store.images.write().unwrap().extend([
        filename(0, "tkecole"),
        filename(58, "tkecole"),
        filename(64, "tkecole"),
        filename(176, "tkecole"),
        filename(183, "tkecole"),
        filename(338, "tkecole"),
        filename(5 * 24 * 60, "tkecole"),
        filename(180, "other"),
        filename(300, "other"),
    ]);
    let gallery = store.gallery(None, None, Some("tkecole"));
    assert_eq!(
        gallery.recent[0].as_ref().unwrap().filename,
        filename(58, "tkecole")
    );
    assert_eq!(
        gallery.recent[1].as_ref().unwrap().filename,
        filename(183, "tkecole")
    );
    assert!(gallery.recent[2].is_none());
    store
        .images
        .write()
        .unwrap()
        .remove(&filename(183, "tkecole"));
    let gallery = store.gallery(None, None, Some("tkecole"));
    assert_eq!(
        gallery.recent[1].as_ref().unwrap().filename,
        filename(176, "tkecole")
    );
    store
        .images
        .write()
        .unwrap()
        .insert(filename(273, "tkecole"));
    let gallery = store.gallery(None, None, Some("tkecole"));
    assert_eq!(
        gallery.recent[2].as_ref().unwrap().filename,
        filename(273, "tkecole")
    );
    assert!(
        store
            .gallery(None, None, Some("missing"))
            .recent
            .iter()
            .all(Option::is_none)
    );
}

#[test]
fn filenames_require_canonical_timestamps_and_safe_accounts() {
    for invalid in [
        "2026-11-02T12:00:00Z_station.jpg",
        "2026-11-02T12:00:00.000000000+01:00_station.jpg",
        "2026-11-02T12:00:00.000000000Z_../station.jpg",
        "2026-11-02T12:00:00.000000000Z_station.svg",
        "2026-11-02T12:00:00.000000000Z_station.jpg/..",
    ] {
        assert!(Frame::parse(invalid).is_none(), "{invalid}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn archive_does_not_serve_symbolic_links() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let outside = tempfile::NamedTempFile::new()?;
    let name = "2026-11-02T12:00:00.000000000Z_station.png";
    let path = directory.path().join(name);
    std::os::unix::fs::symlink(outside.path(), &path)?;
    let store = Store::open(directory.path().into(), false).await?;
    assert!(store.get(name).is_none());
    assert!(open_image(&path).await.is_err());
    Ok(())
}
