use anyhow::Result;
use std::env;

const DEFAULT_ADDRESS: &str = "127.0.0.1:3000";

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub public_base_url: reqwest::Url,
    pub listen_address: String,
    pub database_url: String,
    pub helloasso_client_id: String,
    pub helloasso_client_secret: String,
    pub helloasso_association_slug: String,
    pub mailchimp_api_key: String,
    pub mailchimp_server_prefix: String,
    pub mailchimp_list_id: String,
    pub mailchimp_from_name: String,
    pub mailchimp_from_email: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: String,
    pub smtp_password: String,
    pub smtp_from: String,
    pub gmail_client_id: String,
    pub gmail_client_secret: String,
    pub gmail_redirect_uri: String,
    pub gmail_access_token: String,
    pub gmail_refresh_token: String,
    pub gmail_from: String,
    pub mail_method: String,
    pub mail_destination_override: String,
    pub cookie_secret: String,
    pub sync_token: String,
    pub backup_token: String,
    pub huggingface_token: String,
    pub rss_news_feed: String,
    pub entity_name: String,
    pub entity_email: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let database_url =
            env::var("DATABASE_URL").map_err(|_| anyhow::anyhow!("DATABASE_URL must be set"))?;

        Ok(Self {
            public_base_url: parse_public_url(
                &env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "http://localhost:3000".into()),
            )?,
            listen_address: env::var("LISTEN_ADDRESS").unwrap_or(DEFAULT_ADDRESS.into()),
            database_url,
            helloasso_client_id: env::var("HELLOASSO_CLIENT_ID")
                .map_err(|_| anyhow::anyhow!("HELLOASSO_CLIENT_ID must be set"))?,
            helloasso_client_secret: env::var("HELLOASSO_CLIENT_SECRET")
                .map_err(|_| anyhow::anyhow!("HELLOASSO_CLIENT_SECRET must be set"))?,
            helloasso_association_slug: env::var("HELLOASSO_ASSOCIATION_SLUG")
                .map_err(|_| anyhow::anyhow!("HELLOASSO_ASSOCIATION_SLUG must be set"))?,
            mailchimp_api_key: env::var("MAILCHIMP_API_KEY").unwrap_or_default(),
            mailchimp_server_prefix: env::var("MAILCHIMP_SERVER_PREFIX").unwrap_or_default(),
            mailchimp_list_id: env::var("MAILCHIMP_LIST_ID").unwrap_or_default(),
            mailchimp_from_name: env::var("MAILCHIMP_FROM_NAME").unwrap_or_default(),
            mailchimp_from_email: env::var("MAILCHIMP_FROM_EMAIL").unwrap_or_default(),
            smtp_host: env::var("SMTP_HOST").unwrap_or_default(),
            smtp_port: env::var("SMTP_PORT")
                .unwrap_or_else(|_| "587".to_string())
                .parse()
                .unwrap_or(587),
            smtp_user: env::var("SMTP_USER").unwrap_or_default(),
            smtp_password: env::var("SMTP_PASSWORD").unwrap_or_default(),
            smtp_from: env::var("SMTP_FROM").unwrap_or_default(),
            gmail_client_id: env::var("GMAIL_CLIENT_ID").unwrap_or_default(),
            gmail_client_secret: env::var("GMAIL_CLIENT_SECRET").unwrap_or_default(),
            gmail_redirect_uri: env::var("GMAIL_REDIRECT_URI").unwrap_or_default(),
            gmail_access_token: env::var("GMAIL_ACCESS_TOKEN").unwrap_or_default(),
            gmail_refresh_token: env::var("GMAIL_REFRESH_TOKEN").unwrap_or_default(),
            gmail_from: env::var("GMAIL_FROM").unwrap_or_default(),
            mail_method: env::var("MAIL_METHOD").unwrap_or_default(),
            mail_destination_override: env::var("MAIL_DESTINATION_ADDRESS_OVERRIDE")
                .unwrap_or_default(),
            cookie_secret: env::var("COOKIE_SECRET").unwrap_or_default(),
            sync_token: env::var("SYNC_TOKEN").unwrap_or_default(),
            backup_token: env::var("BACKUP_TOKEN").unwrap_or_default(),
            huggingface_token: env::var("HUGGINGFACE_TOKEN").unwrap_or_default(),
            rss_news_feed: env::var("RSS_NEWS_FEED").unwrap_or_default(),
            entity_name: env::var("ENTITY_NAME").unwrap_or_else(|_| "AG'HIL".into()),
            entity_email: env::var("ENTITY_EMAIL").unwrap_or_default(),
        })
    }
}

fn parse_public_url(value: &str) -> Result<reqwest::Url> {
    let mut url = reqwest::Url::parse(value)?;
    anyhow::ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "PUBLIC_BASE_URL must be an HTTP(S) URL without credentials, query, or fragment"
    );
    url.set_path(&format!("{}/", url.path().trim_end_matches('/')));
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_links_preserve_deployment_path() {
        let base = parse_public_url("https://station.example/powpow").unwrap();
        assert_eq!(
            base.join("person/123?token=secret").unwrap().as_str(),
            "https://station.example/powpow/person/123?token=secret"
        );
        for bad in [
            "javascript:alert(1)",
            "https://user:pass@example.org",
            "https://example.org/?x=y",
            "https://example.org/#fragment",
        ] {
            assert!(parse_public_url(bad).is_err());
        }
    }
}
