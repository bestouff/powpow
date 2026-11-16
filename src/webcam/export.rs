//! Timestamp-paced H.264/MP4 exports, encoded one image at a time on a worker.

use std::io::{Read, Seek};

use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Local, NaiveDate, NaiveTime, TimeZone, Utc};
use filmcraft_h264enc::{Encoder, EncoderConfig, PacketFormat, Preset, Profile, YuvFrame};

use super::{Frame, MAX_IMAGE_BYTES, Store, decode_image};

pub const DEFAULT_SPEED: u32 = 2_000;
pub const MAX_SPEED: u32 = 10_000;
const TIMESCALE: u32 = 1_000;
const MIN_FRAME_MS: u32 = 40;

pub struct Interval {
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    speed: u32,
}

impl Interval {
    pub fn new(
        date: NaiveDate,
        start: NaiveTime,
        end: NaiveTime,
        speed: u32,
        now: DateTime<Utc>,
    ) -> Result<Self> {
        ensure!(
            (1..=MAX_SPEED).contains(&speed),
            "La vitesse doit être comprise entre 1 et {MAX_SPEED}."
        );
        ensure!(
            date <= now.with_timezone(&Local).date_naive(),
            "Choisissez une date passée ou la date du jour."
        );
        let start = local_time(date, start)?;
        let end = local_time(date, end)?.min(now);
        ensure!(
            end > start,
            "L’heure de fin doit être après le début et le début doit être dans le passé."
        );
        ensure!(
            (end - start).num_milliseconds() / i64::from(speed) >= i64::from(MIN_FRAME_MS),
            "La plage est trop courte pour cette vitesse."
        );
        Ok(Self { start, end, speed })
    }
}

fn local_time(date: NaiveDate, time: NaiveTime) -> Result<DateTime<Utc>> {
    Local.from_local_datetime(&date.and_time(time)).single()
        .map(|time| time.with_timezone(&Utc))
        .context("Cette heure est absente ou ambiguë lors du changement d’heure. Choisissez une autre heure.")
}

struct Sample {
    frame: Frame,
    start_ms: u32,
    duration_ms: u32,
}

pub struct Plan(Vec<Sample>);

impl Plan {
    pub fn new(frames: Vec<Frame>, interval: &Interval) -> Result<Self> {
        let duration_ms = u32::try_from(
            (interval.end - interval.start).num_milliseconds() / i64::from(interval.speed),
        )?;
        let mut samples: Vec<Sample> = Vec::new();
        for frame in frames
            .into_iter()
            .filter(|frame| frame.received_at >= interval.start && frame.received_at < interval.end)
        {
            let start_ms = u32::try_from(
                (frame.received_at - interval.start).num_milliseconds() / i64::from(interval.speed),
            )?;
            if let Some(previous) = samples.last_mut() {
                // Thin dense captures to at most 25 fps; preserve elapsed time, including gaps.
                if start_ms - previous.start_ms < MIN_FRAME_MS
                    || duration_ms - start_ms < MIN_FRAME_MS
                {
                    continue;
                }
                previous.duration_ms = start_ms - previous.start_ms;
            }
            let start_ms = if samples.is_empty() { 0 } else { start_ms };
            samples.push(Sample {
                frame,
                start_ms,
                duration_ms: duration_ms - start_ms,
            });
        }
        ensure!(
            !samples.is_empty(),
            "Aucune image pour cette caméra dans la plage choisie."
        );
        Ok(Self(samples))
    }
}

impl Store {
    pub async fn export_video(&self, plan: Plan) -> Result<std::fs::File> {
        let permit = self
            .exports
            .clone()
            .try_acquire_owned()
            .context("Un export est déjà en cours. Réessayez dans quelques instants.")?;
        let store = self.clone();
        let (sender, receiver) = tokio::sync::oneshot::channel();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            // Dropping the HTTP request closes the receiver and stops the next frame.
            let result = encode(&store, &plan, || sender.is_closed());
            let _ = sender.send(result);
        });
        receiver
            .await
            .context("Le traitement vidéo a été interrompu.")?
    }
}

fn read_image(store: &Store, frame: &Frame) -> Result<image::DynamicImage> {
    let mut bytes = Vec::new();
    std::fs::File::open(store.path(frame))?
        .take(MAX_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    let extension = frame
        .filename
        .rsplit_once('.')
        .context("Missing image extension")?
        .1;
    decode_image(&bytes, extension)
}

fn encode(store: &Store, plan: &Plan, cancelled: impl Fn() -> bool) -> Result<std::fs::File> {
    ensure!(!cancelled(), "Export cancelled");
    let first = read_image(store, &plan.0[0].frame)?;
    let first = first.thumbnail(first.width().min(1280), first.height().min(720));
    let width = (first.width() & !1).max(2);
    let height = (first.height() & !1).max(2);
    let mut config = EncoderConfig::new(width, height, 25, 1);
    config.profile = Profile::Baseline;
    config.preset = Preset::Speed;
    config.bframes = 0;
    config.threads = 1;
    config.format = PacketFormat::LengthPrefixed;
    let mut encoder = Encoder::new(config)?;
    let (seq_param_set, pic_param_set) = encoder.sps_pps();
    let mut writer = mp4::Mp4Writer::write_start(
        std::io::BufWriter::new(tempfile::tempfile()?),
        &mp4::Mp4Config {
            major_brand: "isom".parse()?,
            minor_version: 512,
            compatible_brands: vec![
                "isom".parse()?,
                "iso2".parse()?,
                "avc1".parse()?,
                "mp41".parse()?,
            ],
            timescale: TIMESCALE,
        },
    )?;
    writer.add_track(&mp4::TrackConfig::from(mp4::AvcConfig {
        width: width.try_into()?,
        height: height.try_into()?,
        seq_param_set,
        pic_param_set,
    }))?;
    for (index, sample) in plan.0.iter().enumerate() {
        ensure!(!cancelled(), "Export cancelled");
        let image = if index == 0 {
            first.clone()
        } else {
            read_image(store, &sample.frame)?
        };
        let rgb = letterbox(&image, width, height);
        let (y, u, v) = yuv420(&rgb);
        let packets = encoder.encode(
            &YuvFrame {
                y: &y,
                u: &u,
                v: &v,
                y_stride: width as usize,
                uv_stride: width as usize / 2,
            },
            i64::from(sample.start_ms),
        )?;
        ensure!(
            packets.len() == 1,
            "Expected one packet per picture without B-frames"
        );
        for packet in packets {
            writer.write_sample(
                1,
                &mp4::Mp4Sample {
                    start_time: u64::from(sample.start_ms),
                    duration: sample.duration_ms,
                    rendering_offset: 0,
                    is_sync: packet.keyframe,
                    bytes: packet.data.into(),
                },
            )?;
        }
    }
    ensure!(
        encoder.flush().is_empty(),
        "Unexpected buffered video pictures"
    );
    writer.write_end()?;
    let mut file = writer.into_writer().into_inner()?;
    file.rewind()?;
    Ok(file)
}

fn letterbox(image: &image::DynamicImage, width: u32, height: u32) -> image::RgbImage {
    let resized = image
        .resize(width, height, image::imageops::FilterType::Triangle)
        .to_rgb8();
    let mut canvas = image::RgbImage::new(width, height);
    image::imageops::replace(
        &mut canvas,
        &resized,
        i64::from((width - resized.width()) / 2),
        i64::from((height - resized.height()) / 2),
    );
    canvas
}

fn yuv420(image: &image::RgbImage) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let width = image.width() as usize;
    let height = image.height() as usize;
    let mut luma = vec![0; width * height];
    let mut blue_chroma = vec![0; width * height / 4];
    let mut red_chroma = vec![0; width * height / 4];
    // Limited-range BT.709, matching the encoder's colour signalling.
    for row in (0..height).step_by(2) {
        for column in (0..width).step_by(2) {
            let mut chroma_u = 0;
            let mut chroma_v = 0;
            for dy in 0..2 {
                for dx in 0..2 {
                    let index = (row + dy) * width + column + dx;
                    let [r, g, b] = image.as_raw()[index * 3..index * 3 + 3] else {
                        unreachable!()
                    };
                    let (r, g, b) = (i32::from(r), i32::from(g), i32::from(b));
                    luma[index] = byte(16 + ((11_966 * r + 40_254 * g + 4_064 * b + 32_768) >> 16));
                    chroma_u += -6_596 * r - 22_189 * g + 28_785 * b;
                    chroma_v += 28_785 * r - 26_145 * g - 2_640 * b;
                }
            }
            let index = row / 2 * (width / 2) + column / 2;
            blue_chroma[index] = byte(128 + ((chroma_u + 131_072) >> 18));
            red_chroma[index] = byte(128 + ((chroma_v + 131_072) >> 18));
        }
    }
    (luma, blue_chroma, red_chroma)
}

fn byte(value: i32) -> u8 {
    u8::try_from(value.clamp(0, 255)).expect("clamped colour channel")
}

#[cfg(test)]
mod tests;
