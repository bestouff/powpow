//! Retrieve camera snapshots over HTTP and publish them through the shared archive.

use std::time::Duration;

use anyhow::{Context, Result, bail, ensure};
use reqwest::header::{ACCEPT, CACHE_CONTROL};
use tracing::{info, warn};

use super::{Frame, MAX_IMAGE_BYTES, Store, config::DirectConfig};

#[cfg(test)]
#[path = "direct_tests.rs"]
mod tests;

pub struct Fetcher {
    client: reqwest::Client,
    config: DirectConfig,
}

impl Fetcher {
    pub fn new(config: DirectConfig) -> Result<Self> {
        // This administrator-configured URL can also point to a camera on a LAN.
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5))
            .referer(false)
            .build()?;
        Ok(Self { client, config })
    }

    async fn fetch(&self, store: &Store) -> Result<Frame> {
        let mut response = self
            .client
            .get(self.config.url.clone())
            .header(ACCEPT, "image/jpeg, image/png, image/webp")
            .header(CACHE_CONTROL, "no-cache")
            .send()
            .await
            .map_err(reqwest::Error::without_url)
            .context("Direct webcam request failed")?;
        ensure!(
            response.status().is_success(),
            "Direct webcam returned HTTP {}",
            response.status()
        );
        ensure!(
            response
                .content_length()
                .is_none_or(|length| length <= MAX_IMAGE_BYTES),
            "Direct webcam image exceeds 20 MiB"
        );
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(reqwest::Error::without_url)?
        {
            ensure!(
                chunk.len() as u64 <= MAX_IMAGE_BYTES - bytes.len() as u64,
                "Direct webcam image exceeds 20 MiB"
            );
            bytes.extend_from_slice(&chunk);
        }
        // Snapshot URLs often have no extension, and camera MIME types can be wrong.
        let extension =
            match image::guess_format(&bytes).context("Direct webcam response is not an image")? {
                image::ImageFormat::Jpeg => "jpg",
                image::ImageFormat::Png => "png",
                image::ImageFormat::WebP => "webp",
                _ => bail!("Direct webcam image must be JPEG, PNG, or WebP"),
            };
        store
            .save(bytes, self.config.name.clone(), extension.into())
            .await
    }

    /// Fetch immediately, then at the configured interval without overlapping requests.
    pub async fn poll(self, store: Store) {
        info!(period = ?self.config.period, "Direct webcam retrieval enabled");
        let mut interval = tokio::time::interval(self.config.period);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Err(error) = self.fetch(&store).await {
                warn!(error = %format!("{error:#}"), "Direct webcam retrieval failed; will retry next cycle");
            }
        }
    }
}
