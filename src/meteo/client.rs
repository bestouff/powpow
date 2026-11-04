use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use reqwest::{
    StatusCode,
    header::{AUTHORIZATION, HeaderValue},
};
use serde::Deserialize;

use super::{Config, Observation, parse_observations};

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;

const OBSERVATIONS_URL: &str =
    "https://public-api.meteofrance.fr/public/DPObs/v2/station/infrahoraire-6m";
const TOKEN_URL: &str = "https://portail-api.meteofrance.fr/token";

struct AccessToken {
    header: HeaderValue,
    expires_at: Option<Instant>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    token_type: String,
}

pub struct Client {
    http: reqwest::Client,
    station_id: String,
    application: Option<HeaderValue>,
    token: Option<AccessToken>,
    observations_url: String,
    token_url: String,
}

fn authorization(scheme: &str, credential: &str) -> Result<HeaderValue> {
    let mut header = HeaderValue::from_str(&format!("{scheme} {credential}"))
        .context("Invalid Météo-France credential header")?;
    header.set_sensitive(true);
    Ok(header)
}

impl Client {
    pub fn new(config: Config) -> Result<Self> {
        let application = (!config.application_id.is_empty())
            .then(|| authorization("Basic", &config.application_id))
            .transpose()?;
        // Application credentials take precedence over a manually copied token.
        let token = if application.is_none() && !config.token.is_empty() {
            Some(AccessToken {
                header: authorization("Bearer", &config.token)?,
                expires_at: None,
            })
        } else {
            None
        };
        Ok(Self {
            http: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
            station_id: config.station_id,
            application,
            token,
            observations_url: OBSERVATIONS_URL.into(),
            token_url: TOKEN_URL.into(),
        })
    }

    pub fn station_id(&self) -> &str {
        &self.station_id
    }

    async fn renew_token(&mut self) -> Result<()> {
        let Some(application) = &self.application else {
            bail!(
                "METEO_FRANCE_TOKEN is invalid or expired; configure METEO_FRANCE_APPLICATION_ID for automatic renewal, or replace the token and restart PowPow"
            );
        };
        let requested_at = Instant::now();
        let response = self
            .http
            .post(&self.token_url)
            .header(AUTHORIZATION, application.clone())
            .form(&[("grant_type", "client_credentials")])
            .send()
            .await
            .context("Météo-France token request failed")?;
        ensure!(
            response.status().is_success(),
            "Météo-France token request returned HTTP {}; check METEO_FRANCE_APPLICATION_ID and the API subscription",
            response.status()
        );
        let token: TokenResponse = json_body(response, 64 * 1024).await?;
        ensure!(
            !token.access_token.is_empty()
                && token.expires_in > 0
                && token.token_type.eq_ignore_ascii_case("bearer"),
            "Invalid Météo-France OAuth2 token response"
        );
        let lifetime = Duration::from_secs(token.expires_in.saturating_sub(30).max(1));
        self.token = Some(AccessToken {
            header: authorization("Bearer", &token.access_token)?,
            expires_at: Some(
                requested_at
                    .checked_add(lifetime)
                    .context("Invalid OAuth2 token lifetime")?,
            ),
        });
        Ok(())
    }

    async fn request_observations(&self) -> Result<reqwest::Response> {
        let token = self
            .token
            .as_ref()
            .context("Missing Météo-France access token")?;
        self.http
            .get(&self.observations_url)
            .header(AUTHORIZATION, token.header.clone())
            .query(&[("id_station", self.station_id.as_str()), ("format", "json")])
            .send()
            .await
            .context("Météo-France observation request failed")
    }

    pub async fn observations(&mut self) -> Result<Vec<Observation>> {
        if self.token.as_ref().is_none_or(|token| {
            token
                .expires_at
                .is_some_and(|expiry| expiry <= Instant::now())
        }) {
            self.renew_token().await?;
        }
        let mut response = self.request_observations().await?;
        if response.status() == StatusCode::UNAUTHORIZED {
            if self.application.is_some() {
                self.token = None;
            }
            self.renew_token().await?;
            response = self.request_observations().await?;
        }
        ensure!(
            response.status().is_success(),
            "Météo-France observation request returned HTTP {}",
            response.status()
        );
        parse_observations(json_body(response, 1024 * 1024).await?, &self.station_id)
    }
}

async fn json_body<T: serde::de::DeserializeOwned>(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<T> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            chunk.len() <= limit - body.len(),
            "Météo-France response exceeds size limit"
        );
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).context("Invalid Météo-France JSON response")
}
