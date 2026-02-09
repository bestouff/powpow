use crate::models::{
    HelloAssoEvent, HelloAssoEventResponse, HelloAssoOrder, HelloAssoOrdersResponse,
    HelloAssoTokenResponse, HelloAssoUser, HelloAssoUserResponse,
};
use anyhow::{Result, anyhow};
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct HelloAssoClient {
    client: Client,
    client_id: String,
    client_secret: String,
    association_slug: String,
    token_cache: Arc<RwLock<Option<TokenCache>>>,
    rate_limiter: Arc<Semaphore>,
    last_request_time: Arc<RwLock<Instant>>,
}

#[derive(Debug, Clone)]
struct TokenCache {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: chrono::DateTime<chrono::Utc>,
}

impl HelloAssoClient {
    pub fn new(client_id: String, client_secret: String, association_slug: String) -> Self {
        Self {
            client: Client::new(),
            client_id,
            client_secret,
            association_slug,
            token_cache: Arc::new(RwLock::new(None)),
            rate_limiter: Arc::new(Semaphore::new(5)), // 5 requests per rate limit window
            last_request_time: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Get the association slug (public getter)
    pub fn association_slug(&self) -> &str {
        &self.association_slug
    }

    /// Check if the client is properly configured
    pub fn is_configured(&self) -> bool {
        !self.client_id.is_empty()
            && !self.client_secret.is_empty()
            && !self.association_slug.is_empty()
    }

    async fn check_rate_limit(&self) -> Result<()> {
        let rate_limiter = self.rate_limiter.clone();
        let permit = rate_limiter.acquire().await?;

        // Check time since last request
        let last_time = *self.last_request_time.read().await;
        let elapsed = last_time.elapsed();

        // HelloAsso API rate limit: ~10 requests per second, but we'll be more conservative
        if elapsed < Duration::from_millis(200) {
            // 5 requests per second
            let wait_time = Duration::from_millis(200).saturating_sub(elapsed);
            tokio::time::sleep(wait_time).await;
        }

        // Update last request time
        *self.last_request_time.write().await = Instant::now();

        drop(permit); // Release the permit
        Ok(())
    }

    async fn get_access_token(&self) -> Result<String> {
        // Check if we have a valid cached token
        {
            let cache = self.token_cache.read().await;
            if let Some(token_cache) = &*cache
                && token_cache.expires_at > chrono::Utc::now() + chrono::Duration::minutes(5)
            {
                return Ok(token_cache.access_token.clone());
            }
        }

        // Try to refresh token if we have a refresh token
        {
            let cache = self.token_cache.read().await;
            if let Some(token_cache) = &*cache 
                && let Some(refresh_token) = &token_cache.refresh_token {
                match self.refresh_token(refresh_token).await {
                    Ok(new_token) => return Ok(new_token),
                    Err(e) => {
                        warn!("Failed to refresh token, will try to get new token: {}", e);
                        // Continue to get new token
                    }
                }
            }
        }

        // Get new token
        self.get_new_access_token().await
    }

    async fn get_new_access_token(&self) -> Result<String> {
        info!("Requesting new access token from HelloAsso");

        let mut params = HashMap::new();
        params.insert("client_id", self.client_id.as_str());
        params.insert("client_secret", self.client_secret.as_str());
        params.insert("grant_type", "client_credentials");

        let response = self
            .client
            .post("https://api.helloasso.com/oauth2/token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(
                serde_urlencoded::to_string(&params).unwrap()
            )
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to get access token: {}", error_text));
        }

        let token_response: HelloAssoTokenResponse = response.json().await?;

        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(token_response.expires_in);

        // Cache the token
        {
            let mut cache = self.token_cache.write().await;
            *cache = Some(TokenCache {
                access_token: token_response.access_token.clone(),
                refresh_token: token_response.refresh_token.clone(),
                expires_at,
            });
        }

        Ok(token_response.access_token)
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<String> {
        info!("Refreshing access token");

        let mut params = HashMap::new();
        params.insert("client_id", self.client_id.as_str());
        params.insert("grant_type", "refresh_token");
        params.insert("refresh_token", refresh_token);

        let response = self
            .client
            .post("https://api.helloasso.com/oauth2/token")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(
                serde_urlencoded::to_string(&params).unwrap()
            )
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            warn!("Failed to refresh token: {}", error_text);
            return Err(anyhow!("Failed to refresh token: {}", error_text));
        }

        let token_response: HelloAssoTokenResponse = response.json().await?;

        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(token_response.expires_in);

        // Update cache
        {
            let mut cache = self.token_cache.write().await;
            *cache = Some(TokenCache {
                access_token: token_response.access_token.clone(),
                refresh_token: token_response.refresh_token.clone(),
                expires_at,
            });
        }

        Ok(token_response.access_token)
    }

    pub async fn get_orders(&self) -> Result<Vec<HelloAssoOrder>> {
        let access_token = self.get_access_token().await?;
        let mut all_orders = Vec::new();
        let mut continuation_token: Option<String> = None;
        let page_size = 50;
        let mut page_num = 1;

        loop {
            debug!("Fetching orders page {}", page_num);

            // Apply rate limiting
            self.check_rate_limit().await?;

            let url = format!(
                "https://api.helloasso.com/v5/organizations/{}/orders",
                self.association_slug
            );

            let mut query_params = vec![
                ("pageSize", page_size.to_string()),
                ("withDetails", "true".to_string()), // Include custom fields and options
            ];
            // Note: Not filtering by state to get all orders (Processed, Registered, etc.)

            // Use continuation token if we have one, otherwise use pageIndex
            if let Some(token) = &continuation_token {
                query_params.push(("continuationToken", token.clone()));
            } else {
                query_params.push(("pageIndex", "1".to_string()));
            }

            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", access_token))
                .query(&query_params)
                .send()
                .await?;

            let status = response.status();
            let response_text = response.text().await?;

            if !status.is_success() {
                warn!("Orders API returned error status {}: {}", status, response_text);
                return Err(anyhow!("Failed to fetch orders (status {}): {}", status, response_text));
            }

            debug!("Orders API response page {} (first 500 chars): {}", page_num, &response_text.chars().take(500).collect::<String>());

            let orders_response: HelloAssoOrdersResponse = match serde_json::from_str(&response_text) {
                Ok(resp) => resp,
                Err(e) => {
                    warn!("Failed to parse orders response. Error: {}. Response body: {}", e, response_text);
                    return Err(anyhow!("Failed to parse orders response: {}", e));
                }
            };

            let orders_count = orders_response.data.len();

            // Break if no more orders
            if orders_count == 0 {
                break;
            }

            all_orders.extend(orders_response.data);

            info!(
                "Fetched {} orders from page {} (total: {})",
                orders_count,
                page_num,
                all_orders.len()
            );

            // Check if we have a continuation token for the next page
            if let Some(token) = orders_response.pagination.continuation_token {
                continuation_token = Some(token);
                page_num += 1;
            } else {
                // No continuation token means we've reached the end
                break;
            }
        }

        info!("Total orders fetched: {}", all_orders.len());
        Ok(all_orders)
    }

    pub async fn get_organization_info(&self) -> Result<serde_json::Value> {
        let access_token = self.get_access_token().await?;

        // Apply rate limiting
        self.check_rate_limit().await?;

        let url = format!(
            "https://api.helloasso.com/v5/organizations/{}",
            self.association_slug
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to fetch organization info: {}", error_text));
        }

        let organization_info: serde_json::Value = response.json().await?;
        Ok(organization_info)
    }

    pub async fn get_forms(&self) -> Result<Vec<serde_json::Value>> {
        let access_token = self.get_access_token().await?;

        // Apply rate limiting
        self.check_rate_limit().await?;

        let url = format!(
            "https://api.helloasso.com/v5/organizations/{}/forms",
            self.association_slug
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .query(&[("pageSize", "50")])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to fetch forms: {}", error_text));
        }

        let forms_response: serde_json::Value = response.json().await?;

        if let Some(data) = forms_response.get("data")
            && let Some(forms_array) = data.as_array() {
            return Ok(forms_array.clone());
        }

        Ok(vec![])
    }

    pub async fn get_users(&self) -> Result<Vec<HelloAssoUser>> {
        let access_token = self.get_access_token().await?;
        let mut all_users = Vec::new();
        let mut continuation_token: Option<String> = None;
        let page_size = 50;
        let mut page_num = 1;

        loop {
            debug!("Fetching users page {}", page_num);

            // Apply rate limiting
            self.check_rate_limit().await?;

            let url = format!(
                "https://api.helloasso.com/v5/organizations/{}/users",
                self.association_slug
            );

            let mut query_params = vec![("pageSize", page_size.to_string())];

            if let Some(token) = &continuation_token {
                query_params.push(("continuationToken", token.clone()));
            } else {
                query_params.push(("pageIndex", "1".to_string()));
            }

            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", access_token))
                .query(&query_params)
                .send()
                .await?;

            let status = response.status();
            let response_text = response.text().await?;

            if !status.is_success() {
                warn!("Users API returned error status {}: {}", status, response_text);
                return Err(anyhow!("Failed to fetch users (status {}): {}", status, response_text));
            }

            debug!("Users API response (first 500 chars): {}", &response_text.chars().take(500).collect::<String>());

            let users_response: HelloAssoUserResponse = match serde_json::from_str(&response_text) {
                Ok(resp) => resp,
                Err(e) => {
                    warn!("Failed to parse users response. Error: {}. Response body: {}", e, response_text);
                    return Err(anyhow!("Failed to parse users response: {}", e));
                }
            };

            let users_count = users_response.data.len();

            if users_count == 0 {
                break;
            }

            all_users.extend(users_response.data);

            info!(
                "Fetched {} users from page {} (total: {})",
                users_count,
                page_num,
                all_users.len()
            );

            if let Some(token) = users_response.pagination.continuation_token {
                continuation_token = Some(token);
                page_num += 1;
            } else {
                break;
            }
        }

        info!("Total users fetched: {}", all_users.len());
        Ok(all_users)
    }

    pub async fn get_events(&self) -> Result<Vec<HelloAssoEvent>> {
        let access_token = self.get_access_token().await?;
        let mut all_events = Vec::new();
        let mut continuation_token: Option<String> = None;
        let page_size = 50;
        let mut page_num = 1;

        loop {
            debug!("Fetching events page {}", page_num);

            // Apply rate limiting
            self.check_rate_limit().await?;

            let url = format!(
                "https://api.helloasso.com/v5/organizations/{}/events",
                self.association_slug
            );

            let mut query_params = vec![("pageSize", page_size.to_string())];

            if let Some(token) = &continuation_token {
                query_params.push(("continuationToken", token.clone()));
            } else {
                query_params.push(("pageIndex", "1".to_string()));
            }

            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", access_token))
                .query(&query_params)
                .send()
                .await?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await?;
                return Err(anyhow!("Failed to fetch events: HTTP {} - {}", status, error_text));
            }

            let response_text = response.text().await?;
            let events_response: HelloAssoEventResponse = match serde_json::from_str(&response_text) {
                Ok(resp) => resp,
                Err(e) => {
                    warn!("Failed to parse events response. Error: {}. Response body: {}", e, response_text);
                    return Err(anyhow!("Failed to parse events response: {}", e));
                }
            };

            let events_count = events_response.data.len();

            if events_count == 0 {
                break;
            }

            all_events.extend(events_response.data);

            info!(
                "Fetched {} events from page {} (total: {})",
                events_count,
                page_num,
                all_events.len()
            );

            if let Some(token) = events_response.pagination.continuation_token {
                continuation_token = Some(token);
                page_num += 1;
            } else {
                break;
            }
        }

        info!("Total events fetched: {}", all_events.len());
        Ok(all_events)
    }

    pub async fn get_event_registrations(&self, event_id: i64) -> Result<Vec<HelloAssoUser>> {
        let access_token = self.get_access_token().await?;
        let mut all_users = Vec::new();
        let mut page_index = 1;
        let page_size = 50;

        loop {
            debug!(
                "Fetching event {} registrations page {}",
                event_id,
                page_index
            );

            // Apply rate limiting
            self.check_rate_limit().await?;

            let url = format!(
                "https://api.helloasso.com/v5/organizations/{}/events/{}/registrations",
                self.association_slug, event_id
            );

            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", access_token))
                .query(&[
                    ("pageIndex", page_index.to_string()),
                    ("pageSize", page_size.to_string()),
                ])
                .send()
                .await?;

            if !response.status().is_success() {
                let error_text = response.text().await?;
                return Err(anyhow!(
                    "Failed to fetch event registrations: {}",
                    error_text
                ));
            }

            let users_response: HelloAssoUserResponse = response.json().await?;

            let users_count = users_response.data.len();
            all_users.extend(users_response.data);

            info!(
                "Fetched {} registrations for event {} from page {} (total: {})",
                users_count,
                event_id,
                page_index,
                all_users.len()
            );

            // Check if we have more pages
            if page_index >= users_response.pagination.total_pages {
                break;
            }

            page_index += 1;
        }

        info!(
            "Total registrations fetched for event {}: {}",
            event_id,
            all_users.len()
        );
        Ok(all_users)
    }
}
