use std::{net::SocketAddr, ops::RangeInclusive, path::PathBuf};

use anyhow::{Context, Result, ensure};

#[derive(Clone, Debug)]
pub struct WebcamConfig {
    pub title: String,
    pub directory: PathBuf,
    pub ftps: Option<FtpsConfig>,
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
