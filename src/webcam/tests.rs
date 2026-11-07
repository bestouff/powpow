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
    assert_eq!(directory.path().read_dir()?.count(), 2);
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
    assert_eq!(directory.path().read_dir()?.count(), 2);

    std::fs::write(directory.path().join(".upload-unfinished"), png())?;
    std::fs::write(directory.path().join("unrelated.png"), png())?;
    let restarted = Store::open(directory.path().into(), false).await?;
    assert_eq!(
        restarted.latest().unwrap().filename,
        store.latest().unwrap().filename
    );
    assert_eq!(restarted.gallery(None, None, 0).images.len(), 2);
    assert!(restarted.get("../outside.png").is_none());
    assert!(restarted.get(".upload-unfinished").is_none());
    Ok(())
}

#[tokio::test]
async fn gallery_paginates_a_day_and_navigates_across_midnight() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let midnight = Local
        .with_ymd_and_hms(2026, 11, 2, 0, 0, 0)
        .unwrap()
        .with_timezone(&Utc);
    for minute in 0..55 {
        let received_at = midnight - TimeDelta::hours(1) + TimeDelta::minutes(minute);
        std::fs::write(
            directory.path().join(format!(
                "{}_station.jpg",
                received_at.to_rfc3339_opts(SecondsFormat::Nanos, true)
            )),
            [],
        )?;
    }
    let newest = format!(
        "{}_station.jpg",
        midnight.to_rfc3339_opts(SecondsFormat::Nanos, true)
    );
    std::fs::write(directory.path().join(&newest), [])?;
    let store = Store::open(directory.path().into(), false).await?;
    let day = "2026-11-01".parse()?;
    let latest = store.gallery(None, None, 0);
    assert_eq!(latest.selected.unwrap().filename, newest);
    assert!(latest.next.is_none());
    assert_eq!(
        latest
            .previous
            .unwrap()
            .received_at
            .with_timezone(&Local)
            .date_naive(),
        day
    );
    let first = store.gallery(Some(day), None, 0);
    assert_eq!(first.images.len(), PAGE_SIZE);
    assert_eq!(first.pages, 2);
    let last = store.gallery(Some(day), None, usize::MAX);
    assert_eq!(last.page, 1);
    assert_eq!(last.images.len(), 7);
    let chosen = store.gallery(None, Some(&last.images[6].filename), 0);
    assert_eq!(chosen.page, 1);
    assert_eq!(chosen.date, day);
    assert!(chosen.previous.is_none());
    assert!(
        store
            .gallery(Some("2026-10-30".parse()?), None, 0)
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
                "{}_station.jpg",
                received_at.to_rfc3339_opts(SecondsFormat::Nanos, true)
            );
            std::fs::write(directory.path().join(&filename), [])?;
            if received_at >= start && received_at < end {
                expected.push(filename);
            }
            received_at += step;
        }
        expected.reverse();
        let store = Store::open(directory.path().into(), false).await?;
        let first = store.gallery(Some(date), None, 0);
        let actual: Vec<_> = (0..first.pages)
            .flat_map(|page| store.gallery(Some(date), None, page).images)
            .map(|frame| frame.filename)
            .collect();
        assert_eq!(actual, expected, "{date}");
        let earliest = store.gallery(None, expected.last().map(String::as_str), 0);
        assert_eq!(earliest.date, date);
        assert_eq!(earliest.previous.unwrap().received_at, start - step);
        assert_eq!(first.next.unwrap().received_at, end);
    }
    Ok(())
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
