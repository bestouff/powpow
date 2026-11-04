use std::{
    net::SocketAddr,
    ops::RangeInclusive,
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};

#[derive(Clone, Debug)]
pub struct WebcamConfig {
    pub title: String,
    pub directory: PathBuf,
    pub ftps: Option<FtpsConfig>,
    pub direct: Option<DirectConfig>,
}

#[derive(Clone)]
pub struct DirectConfig {
    pub url: reqwest::Url,
    pub period: Duration,
    pub name: String,
    pub title: Option<String>,
}

impl std::fmt::Debug for DirectConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Camera URLs may contain credentials or query-string access tokens.
        formatter
            .debug_struct("DirectConfig")
            .field("period", &self.period)
            .field("name", &self.name)
            .field("title", &self.title)
            .finish_non_exhaustive()
    }
}

impl DirectConfig {
    fn parse(url: &str, period: &str, name: &str, title: &str) -> Result<Option<Self>> {
        let url = url.trim();
        let period = period.trim();
        if url.is_empty() && period.is_empty() {
            return Ok(None);
        }
        ensure!(
            !url.is_empty() && !period.is_empty(),
            "DIRECT_WEBCAM_URL and DIRECT_WEBCAM_PERIOD must be set together"
        );
        let url = reqwest::Url::parse(url).context("Invalid DIRECT_WEBCAM_URL")?;
        ensure!(
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.fragment().is_none(),
            "DIRECT_WEBCAM_URL must be an HTTP(S) URL without a fragment"
        );
        let unit_start = period
            .find(|character: char| !character.is_ascii_digit())
            .context("DIRECT_WEBCAM_PERIOD needs a unit: s, m, h, or d (for example, 6m)")?;
        let (amount, unit) = period.split_at(unit_start);
        let amount: u64 = amount
            .parse()
            .context("DIRECT_WEBCAM_PERIOD must start with a positive whole number")?;
        let multiplier = match unit.trim() {
            "s" => 1,
            "m" => 60,
            "h" => 3_600,
            "d" => 86_400,
            _ => {
                anyhow::bail!("DIRECT_WEBCAM_PERIOD needs a unit: s, m, h, or d (for example, 6m)")
            }
        };
        let seconds = amount
            .checked_mul(multiplier)
            .context("DIRECT_WEBCAM_PERIOD is too large")?;
        ensure!(
            seconds > 0,
            "DIRECT_WEBCAM_PERIOD must be greater than zero"
        );
        let period = Duration::from_secs(seconds);
        ensure!(
            Instant::now().checked_add(period).is_some(),
            "DIRECT_WEBCAM_PERIOD is too large"
        );
        let name = if name.trim().is_empty() {
            "direct"
        } else {
            name.trim()
        };
        ensure!(
            valid_username(name),
            "DIRECT_WEBCAM_NAME must contain 1–64 ASCII letters, digits, hyphens, or underscores"
        );
        let title = title.trim();
        Ok(Some(Self {
            url,
            period,
            name: name.into(),
            title: (!title.is_empty()).then(|| title.into()),
        }))
    }
}

#[derive(Clone)]
pub struct FtpsConfig {
    pub username: String,
    pub password: String,
    pub listen_address: SocketAddr,
    pub passive_ports: RangeInclusive<u16>,
    pub passive_host: thirtyftp::PassiveHost,
    pub certificate: PathBuf,
    pub private_key: PathBuf,
}

impl std::fmt::Debug for FtpsConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FtpsConfig")
            .field("username", &self.username)
            .field("listen_address", &self.listen_address)
            .finish_non_exhaustive()
    }
}

pub fn valid_username(username: &str) -> bool {
    (1..=64).contains(&username.len())
        && username
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

impl WebcamConfig {
    pub fn from_env() -> Result<Self> {
        Self::parse(|key| std::env::var(key).unwrap_or_default())
    }

    fn parse(value: impl Fn(&str) -> String) -> Result<Self> {
        let direct = DirectConfig::parse(
            &value("DIRECT_WEBCAM_URL"),
            &value("DIRECT_WEBCAM_PERIOD"),
            &value("DIRECT_WEBCAM_NAME"),
            &value("DIRECT_WEBCAM_TITLE"),
        )?;
        let title = value("FTPS_TITLE");
        let title = if title.trim().is_empty() {
            "Webcam".into()
        } else {
            title.trim().to_string()
        };
        let directory = value("WEBCAM_DIRECTORY");
        let directory = if directory.is_empty() {
            PathBuf::from("/var/lib/powpow/webcam")
        } else {
            PathBuf::from(directory)
        };
        ensure!(
            directory.is_absolute(),
            "WEBCAM_DIRECTORY must be an absolute path"
        );
        let username = value("FTPS_USERNAME");
        let password = value("FTPS_PASSWORD");
        if username.is_empty() && password.is_empty() {
            return Ok(Self {
                title,
                directory,
                ftps: None,
                direct,
            });
        }
        ensure!(
            valid_username(&username),
            "FTPS_USERNAME must contain 1–64 ASCII letters, digits, hyphens, or underscores"
        );
        ensure!(
            !password.is_empty(),
            "FTPS_PASSWORD must be set with FTPS_USERNAME"
        );
        ensure!(
            !password.chars().any(char::is_control),
            "FTPS_PASSWORD must not contain control characters"
        );

        let address = value("FTPS_LISTEN_ADDRESS");
        let listen_address: SocketAddr = if address.is_empty() {
            "0.0.0.0:2121"
        } else {
            &address
        }
        .parse()
        .context("FTPS_LISTEN_ADDRESS must be an IP address and port")?;
        ensure!(
            listen_address.port() > 0,
            "FTPS_LISTEN_ADDRESS must specify a nonzero port"
        );

        let ports = value("FTPS_PASSIVE_PORTS");
        let ports = if ports.is_empty() {
            "50000-50009"
        } else {
            &ports
        };
        let (first, last) = ports
            .split_once('-')
            .context("FTPS_PASSIVE_PORTS must be a range such as 50000-50009")?;
        let first: u16 = first.parse().context("Invalid first FTPS passive port")?;
        let last: u16 = last.parse().context("Invalid last FTPS passive port")?;
        ensure!(
            first > 0 && first <= last,
            "FTPS_PASSIVE_PORTS must be a nonempty range of nonzero ports"
        );
        let passive_ports = first..=last;
        ensure!(
            !passive_ports.contains(&listen_address.port()),
            "FTPS_PASSIVE_PORTS must exclude the control port"
        );
        let host = value("FTPS_PASSIVE_HOST");
        let passive_host = if host.is_empty() {
            thirtyftp::PassiveHost::FromConnection
        } else if let Ok(ip) = host.parse() {
            thirtyftp::PassiveHost::Ip(ip)
        } else {
            ensure!(
                host.len() <= 253
                    && host
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-')),
                "FTPS_PASSIVE_HOST must be an IPv4 address or DNS hostname"
            );
            thirtyftp::PassiveHost::Dns(host)
        };
        let certificate = value("FTPS_TLS_CERT");
        let private_key = value("FTPS_TLS_KEY");
        ensure!(
            !certificate.is_empty() && !private_key.is_empty(),
            "FTPS_TLS_CERT and FTPS_TLS_KEY must name readable PEM files when FTPS is enabled"
        );
        Ok(Self {
            title,
            directory,
            direct,
            ftps: Some(FtpsConfig {
                username,
                password,
                listen_address,
                passive_ports,
                passive_host,
                certificate: certificate.into(),
                private_key: private_key.into(),
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_configuration_requires_a_url_and_a_positive_period_with_units() -> Result<()> {
        let settings = |url: &str, period: &str| {
            WebcamConfig::parse(|key| match key {
                "DIRECT_WEBCAM_URL" => url.into(),
                "DIRECT_WEBCAM_PERIOD" => period.into(),
                _ => String::new(),
            })
        };
        assert!(settings("", "")?.direct.is_none());
        for (value, seconds) in [
            ("30s", 30),
            ("6m", 360),
            ("1h", 3_600),
            ("2d", 172_800),
            (" 6 m ", 360),
        ] {
            let config = settings("http://192.168.1.2/snapshot?token=secret", value)?;
            assert!(config.ftps.is_none());
            assert_eq!(config.direct.as_ref().unwrap().name, "direct");
            assert!(config.direct.as_ref().unwrap().title.is_none());
            assert_eq!(
                config.direct.as_ref().unwrap().period,
                Duration::from_secs(seconds)
            );
            assert!(!format!("{config:?}").contains("secret"));
        }
        for (url, period) in [
            ("", "6m"),
            ("https://camera.example/snapshot", ""),
            ("file:///tmp/photo.jpg", "6m"),
            ("ftp://camera.example/photo.jpg", "6m"),
            ("https://camera.example/#fragment", "6m"),
            ("not a URL", "6m"),
        ] {
            assert!(settings(url, period).is_err(), "{url}, {period}");
        }
        for period in [
            "0s",
            "0m",
            "6",
            "-1m",
            "1.5m",
            "6ms",
            "m",
            "6M",
            "6w",
            "18446744073709551615d",
            "999999999999999999999s",
        ] {
            assert!(
                settings("https://camera.example/snapshot", period).is_err(),
                "{period}"
            );
        }
        let named_settings = |key: &str| match key {
            "DIRECT_WEBCAM_URL" => "https://camera.example/snapshot".into(),
            "DIRECT_WEBCAM_PERIOD" => "6m".into(),
            "DIRECT_WEBCAM_NAME" => " pistes_nord-1 ".into(),
            "DIRECT_WEBCAM_TITLE" => " Vue <nord> ".into(),
            _ => String::new(),
        };
        let direct = WebcamConfig::parse(named_settings)?.direct.unwrap();
        assert_eq!(direct.name, "pistes_nord-1");
        assert_eq!(direct.title.as_deref(), Some("Vue <nord>"));
        for invalid_name in [
            "../pistes",
            "pistes/nord",
            "pistes nord",
            "été",
            &"a".repeat(65),
        ] {
            let error = WebcamConfig::parse(|key| {
                if key == "DIRECT_WEBCAM_NAME" {
                    invalid_name.into()
                } else {
                    named_settings(key)
                }
            })
            .unwrap_err();
            assert!(error.to_string().contains("DIRECT_WEBCAM_NAME"));
        }
        Ok(())
    }

    #[test]
    fn ftps_configuration_is_opt_in_and_rejects_incomplete_settings() {
        assert!(
            WebcamConfig::parse(|_| String::new())
                .unwrap()
                .ftps
                .is_none()
        );
        let settings = |key: &str| {
            match key {
                "FTPS_TITLE" => "Vue sur les pistes",
                "FTPS_USERNAME" => "station",
                "FTPS_PASSWORD" => "secret",
                "FTPS_TLS_CERT" => "/cert.pem",
                "FTPS_TLS_KEY" => "/key.pem",
                _ => "",
            }
            .to_string()
        };
        assert!(WebcamConfig::parse(settings).unwrap().ftps.is_some());
        let both = WebcamConfig::parse(|key| match key {
            "DIRECT_WEBCAM_URL" => "https://camera.example/snapshot.jpg".into(),
            "DIRECT_WEBCAM_PERIOD" => "6m".into(),
            _ => settings(key),
        })
        .unwrap();
        assert!(both.ftps.is_some() && both.direct.is_some());
        assert_eq!(
            WebcamConfig::parse(settings).unwrap().title,
            "Vue sur les pistes"
        );
        for (key, bad) in [
            ("FTPS_USERNAME", "../station"),
            ("FTPS_USERNAME", ""),
            ("FTPS_PASSWORD", ""),
            ("FTPS_TLS_KEY", ""),
            ("FTPS_PASSIVE_PORTS", "60000-50000"),
            ("FTPS_PASSIVE_PORTS", "2121-2122"),
            ("FTPS_LISTEN_ADDRESS", "localhost"),
            ("FTPS_PASSIVE_HOST", "ftp://station.example"),
            ("WEBCAM_DIRECTORY", "relative/path"),
        ] {
            assert!(
                WebcamConfig::parse(|name| if name == key {
                    bad.to_string()
                } else {
                    settings(name)
                })
                .is_err(),
                "{key}={bad}"
            );
        }
    }
}
