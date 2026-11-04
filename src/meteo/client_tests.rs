use super::*;
use axum::{
    Router,
    body::to_bytes,
    extract::{Request, State},
    response::IntoResponse,
    routing::any,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

struct Expected {
    authorization: &'static str,
    status: StatusCode,
    body: String,
}

fn token(value: &str) -> Expected {
    Expected {
        authorization: "Basic application-secret",
        status: StatusCode::OK,
        body:
            serde_json::json!({"access_token": value, "expires_in": 3600, "token_type": "Bearer"})
                .to_string(),
    }
}

fn observation(authorization: &'static str, status: StatusCode) -> Expected {
    Expected {
        authorization,
        status,
        body: include_str!("fixtures/saint_hilaire.json").into(),
    }
}

struct Mock {
    pending: Arc<Mutex<VecDeque<Expected>>>,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Mock {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn mock(application: &str, expected: Vec<Expected>) -> Result<(Client, Mock)> {
    async fn handle(
        State(pending): State<Arc<Mutex<VecDeque<Expected>>>>,
        request: Request,
    ) -> impl IntoResponse {
        let expected = pending
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected request");
        assert_eq!(request.headers()[AUTHORIZATION], expected.authorization);
        if expected.authorization.starts_with("Basic ") {
            assert_eq!(request.method(), "POST");
            assert_eq!(request.uri().path(), "/token");
            assert_eq!(
                to_bytes(request.into_body(), 1024).await.unwrap(),
                "grant_type=client_credentials"
            );
        } else {
            assert_eq!(request.method(), "GET");
            assert_eq!(request.uri().path(), "/observations");
            assert_eq!(
                request.uri().query(),
                Some("id_station=38395004&format=json")
            );
        }
        (expected.status, expected.body)
    }
    let pending = Arc::new(Mutex::new(VecDeque::from(expected)));
    let app = Router::new()
        .fallback(any(handle))
        .with_state(pending.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut client = Client::new(Config {
        station_id: "38395004".into(),
        token: "manual-secret".into(),
        application_id: application.into(),
    })?;
    client.observations_url = format!("{base}/observations");
    client.token_url = format!("{base}/token");
    Ok((client, Mock { pending, server }))
}

#[tokio::test]
async fn oauth_tokens_are_cached_and_renewed_before_expiry() -> Result<()> {
    let (mut client, mock) = mock(
        "application-secret",
        vec![
            token("first"),
            observation("Bearer first", StatusCode::OK),
            observation("Bearer first", StatusCode::OK),
            token("second"),
            observation("Bearer second", StatusCode::OK),
        ],
    )
    .await?;
    assert_eq!(client.observations().await?.len(), 1);
    assert!(client.application.as_ref().unwrap().is_sensitive());
    assert!(client.token.as_ref().unwrap().header.is_sensitive());
    assert_eq!(client.observations().await?.len(), 1);
    client.token.as_mut().unwrap().expires_at = Some(Instant::now());
    assert_eq!(client.observations().await?.len(), 1);
    assert!(mock.pending.lock().unwrap().is_empty());
    Ok(())
}

#[tokio::test]
async fn unauthorized_renews_once_and_retry_is_bounded() -> Result<()> {
    for final_status in [StatusCode::OK, StatusCode::UNAUTHORIZED] {
        let (mut client, mock) = mock(
            "application-secret",
            vec![
                token("first"),
                observation("Bearer first", StatusCode::UNAUTHORIZED),
                token("second"),
                observation("Bearer second", final_status),
            ],
        )
        .await?;
        assert_eq!(
            client.observations().await.is_ok(),
            final_status.is_success()
        );
        assert!(mock.pending.lock().unwrap().is_empty());
    }
    Ok(())
}

#[tokio::test]
async fn standalone_token_reports_how_to_enable_renewal() -> Result<()> {
    let (mut client, mock) = mock(
        "",
        vec![
            observation("Bearer manual-secret", StatusCode::OK),
            observation("Bearer manual-secret", StatusCode::UNAUTHORIZED),
        ],
    )
    .await?;
    assert_eq!(client.observations().await?.len(), 1);
    let error = client.observations().await.unwrap_err().to_string();
    assert!(error.contains("METEO_FRANCE_APPLICATION_ID"));
    assert!(!error.contains("manual-secret"));
    assert!(mock.pending.lock().unwrap().is_empty());
    Ok(())
}

#[tokio::test]
async fn failed_requests_recover_on_next_poll_without_leaking_response_bodies() -> Result<()> {
    let (mut client, mock) = mock(
        "application-secret",
        vec![
            Expected {
                authorization: "Basic application-secret",
                status: StatusCode::BAD_REQUEST,
                body: "application-secret".into(),
            },
            token("first"),
            observation("Bearer first", StatusCode::TOO_MANY_REQUESTS),
            observation("Bearer first", StatusCode::OK),
        ],
    )
    .await?;
    let error = client.observations().await.unwrap_err().to_string();
    assert!(error.contains("400"));
    assert!(!error.contains("application-secret"));
    assert!(
        client
            .observations()
            .await
            .unwrap_err()
            .to_string()
            .contains("429")
    );
    assert_eq!(client.observations().await?.len(), 1);
    assert!(mock.pending.lock().unwrap().is_empty());
    Ok(())
}

#[tokio::test]
async fn invalid_and_oversized_json_are_rejected() -> Result<()> {
    for body in ["not json".into(), " ".repeat(1024 * 1024 + 1)] {
        let (mut client, mock) = mock(
            "",
            vec![Expected {
                authorization: "Bearer manual-secret",
                status: StatusCode::OK,
                body,
            }],
        )
        .await?;
        assert!(client.observations().await.is_err());
        assert!(mock.pending.lock().unwrap().is_empty());
    }
    Ok(())
}
