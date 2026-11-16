use super::*;
use chrono::TimeDelta;

fn frame(at: DateTime<Utc>) -> Frame {
    Frame {
        filename: format!(
            "{}_station.png",
            at.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
        ),
        received_at: at,
        username: "station".into(),
    }
}

#[test]
fn export_timing_preserves_gaps_and_thins_fast_captures() -> Result<()> {
    let start = "2026-01-15T10:00:00Z".parse()?;
    let interval = Interval {
        start,
        end: start + TimeDelta::minutes(10),
        speed: 100,
    };
    let frames =
        [-1, 10, 12, 120, 540, 599, 600].map(|seconds| frame(start + TimeDelta::seconds(seconds)));
    let plan = Plan::new(frames.to_vec(), &interval)?;
    assert_eq!(
        plan.0
            .iter()
            .map(|sample| (sample.start_ms, sample.duration_ms))
            .collect::<Vec<_>>(),
        [(0, 120), (120, 1080), (1200, 4200), (5400, 600)]
    );
    assert_eq!(
        plan.0.iter().map(|sample| sample.duration_ms).sum::<u32>(),
        6000
    );
    assert!(Plan::new(vec![frame(interval.end)], &interval).is_err());
    Ok(())
}

#[test]
fn export_intervals_validate_and_cap_at_now() -> Result<()> {
    let date: NaiveDate = "2026-01-15".parse()?;
    let start: NaiveTime = "10:00".parse()?;
    let end = "12:00".parse()?;
    let now = local_time(date, "11:00".parse()?)?;
    let interval = Interval::new(date, start, end, 100, now)?;
    assert_eq!(interval.end, now);
    assert_eq!((interval.end - interval.start).num_seconds(), 3600);
    for speed in [0, MAX_SPEED + 1] {
        assert!(Interval::new(date, start, end, speed, now).is_err());
    }
    assert!(Interval::new(date, end, start, 100, now).is_err());
    assert!(Interval::new(date.succ_opt().unwrap(), start, end, 100, now).is_err());
    assert!(Interval::new(date, start, "10:00:01".parse()?, MAX_SPEED, now).is_err());
    // Exercise real DST boundaries when running the suite with TZ=Europe/Paris.
    if std::env::var("TZ").as_deref() == Ok("Europe/Paris") {
        assert!(local_time("2026-03-29".parse()?, "02:30".parse()?).is_err());
        assert!(local_time("2026-10-25".parse()?, "02:30".parse()?).is_err());
        let interval = Interval::new(
            "2026-10-25".parse()?,
            "01:00".parse()?,
            "04:00".parse()?,
            100,
            "2026-10-26T00:00:00Z".parse()?,
        )?;
        assert_eq!((interval.end - interval.start).num_hours(), 4);
    }
    Ok(())
}

#[test]
fn export_colour_conversion_uses_limited_range_and_neutral_chroma() {
    for (rgb, expected_y) in [(0, 16), (255, 235)] {
        let image = image::RgbImage::from_pixel(2, 2, image::Rgb([rgb; 3]));
        let (y, u, v) = yuv420(&image);
        assert_eq!(y, [expected_y; 4]);
        assert_eq!(u, [128]);
        assert_eq!(v, [128]);
    }
}

#[tokio::test]
async fn export_mp4_has_timed_avc_samples_and_releases_worker() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::new(
        directory.path().into(),
        super::super::metadata::Settings::default(),
    );
    let start = "2026-01-15T10:00:00Z".parse()?;
    let interval = Interval {
        start,
        end: start + TimeDelta::minutes(10),
        speed: 100,
    };
    let frames = [10, 120, 540].map(|seconds| frame(start + TimeDelta::seconds(seconds)));
    for (index, frame) in frames.iter().enumerate() {
        let index = u32::try_from(index)?;
        let image = image::RgbImage::from_fn(32 + index * 2, 24, |x, y| {
            image::Rgb([
                u8::try_from(x * 5).unwrap(),
                u8::try_from(y * 8).unwrap(),
                u8::try_from(index * 80).unwrap(),
            ])
        });
        image.save(store.path(frame))?;
    }
    let plan = Plan::new(frames.to_vec(), &interval)?;
    let permit = store.exports.clone().acquire_owned().await?;
    assert!(
        store
            .export_video(Plan::new(frames.to_vec(), &interval)?)
            .await
            .unwrap_err()
            .downcast_ref::<tokio::sync::TryAcquireError>()
            .is_some()
    );
    drop(permit);
    assert!(encode(&store, &plan, || true).is_err());
    let file = store.export_video(plan).await?;
    let length = file.metadata()?.len();
    let mut mp4 = mp4::Mp4Reader::read_header(file, length)?;
    let track = &mp4.tracks()[&1];
    assert_eq!(track.width(), 32);
    assert_eq!(track.height(), 24);
    assert_eq!(track.sample_count(), 3);
    assert_eq!(mp4.duration(), std::time::Duration::from_secs(6));
    for (id, start_ms, duration) in [(1, 0, 1200), (2, 1200, 4200), (3, 5400, 600)] {
        let sample = mp4.read_sample(1, id)?.unwrap();
        assert_eq!(sample.start_time, start_ms);
        assert_eq!(sample.duration, duration);
        assert_eq!(sample.rendering_offset, 0);
        assert!(!sample.bytes.is_empty());
    }
    assert!(mp4.read_sample(1, 1)?.unwrap().is_sync);
    std::fs::remove_file(store.path(&frames[0]))?;
    assert!(
        store
            .export_video(Plan::new(frames.to_vec(), &interval)?)
            .await
            .is_err()
    );
    assert_eq!(store.exports.available_permits(), 1);
    Ok(())
}
