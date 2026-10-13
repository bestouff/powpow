//! Fetch untrusted feed URLs without reaching local services or buffering unbounded bodies.

use anyhow::{Result, ensure};
use reqwest::{
    Client, Url,
    dns::{Addrs, Name, Resolve, Resolving},
};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(matches!(a, 0 | 10 | 127 | 224..=255)
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && (b == 168 || b == 0 || (b == 88 && c == 99)))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            let [a, b, ..] = ip.segments();
            (0x2000..=0x3fff).contains(&a)
                && !(a == 0x2001 && (b < 0x0200 || b == 0x0db8))
                && a != 0x2002
                && !(a == 0x3fff && b <= 0x0fff)
        }
    }
}

fn public_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
        && url.username().is_empty()
        && url.password().is_none()
        && url.host_str().is_some_and(|host| {
            host.trim_matches(['[', ']'])
                .parse::<IpAddr>()
                .map_or(true, public_ip)
        })
}

#[derive(Debug)]
struct PublicDns;

impl Resolve for PublicDns {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let addresses: Vec<SocketAddr> =
                tokio::net::lookup_host((name.as_str(), 0)).await?.collect();
            if addresses.is_empty() || addresses.iter().any(|address| !public_ip(address.ip())) {
                return Err("RSS destination is not a public address".into());
            }
            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}

pub fn client(timeout: Duration) -> Result<Client> {
    Ok(Client::builder()
        .timeout(timeout)
        .no_proxy()
        .dns_resolver(Arc::new(PublicDns))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !public_url(attempt.url()) {
                attempt.error("RSS redirect is not allowed")
            } else {
                attempt.follow()
            }
        }))
        .build()?)
}

pub async fn get(client: &Client, value: &str, limit: usize) -> Result<Vec<u8>> {
    let url = Url::parse(value)?;
    ensure!(
        public_url(&url),
        "RSS destination is not a public HTTP(S) URL"
    );
    bounded_body(client.get(url).send().await?.error_for_status()?, limit).await
}

async fn bounded_body(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
    ensure!(
        response
            .content_length()
            .is_none_or(|size| size <= limit as u64),
        "RSS response exceeds size limit"
    );
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            chunk.len() <= limit - body.len(),
            "RSS response exceeds size limit"
        );
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_internal_literal_urls_and_special_ranges() {
        for url in [
            "http://127.0.0.1",
            "http://2130706433",
            "http://10.0.0.1",
            "http://169.254.169.254",
            "http://100.64.0.1",
            "http://[::1]",
            "http://[::ffff:127.0.0.1]",
            "http://[fc00::1]",
            "http://[2002:7f00:1::]",
            "ftp://example.org",
            "https://user:pass@example.org",
        ] {
            assert!(!public_url(&Url::parse(url).unwrap()), "{url}");
        }
        assert!(public_url(
            &Url::parse("https://example.org/image.png").unwrap()
        ));
        assert!(public_ip("2606:4700::1111".parse().unwrap()));
    }

    #[tokio::test]
    async fn rejects_internal_dns_and_chunked_oversize_bodies() -> Result<()> {
        use tokio::io::AsyncWriteExt;
        assert!(PublicDns.resolve("localhost".parse()?).await.is_err());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await?;
            stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n8\r\n12345678\r\n8\r\n12345678\r\n0\r\n\r\n").await
        });
        let response = Client::new()
            .get(format!("http://{address}"))
            .send()
            .await?;
        assert!(bounded_body(response, 10).await.is_err());
        server.await??;
        Ok(())
    }
}
