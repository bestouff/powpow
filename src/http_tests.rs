use super::*;
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use tower::ServiceExt;

#[test]
fn proxy_prefix_cannot_be_an_external_or_parent_relative_url() {
    let mut headers = HeaderMap::new();
    for prefix in ["//evil.example", "relative", "/../evil", "/a/./b"] {
        headers.insert("x-forwarded-prefix", prefix.parse().unwrap());
        assert_eq!(get_prefix(&headers), "");
    }
    headers.insert("x-forwarded-prefix", "/powpow/".parse().unwrap());
    assert_eq!(get_prefix(&headers), "/powpow");
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn atelier_names_are_escaped_in_reminders_and_calendar_data(
    pool: PgPool,
) -> anyhow::Result<()> {
    let state = state(pool);
    let (chief, cookie) = member(&state, "chief@example.org").await?;
    let (pending, _) = member(&state, "pending@example.org").await?;
    let attack = "</script><script>window.reviewXss=1</script>";
    let atelier: uuid::Uuid = sqlx::query_scalar("INSERT INTO ateliers (name, slug, needs_validation) VALUES ($1, 'test-escaping', true) RETURNING id")
        .bind(attack).fetch_one(&state.db).await?;
    database::add_role(&state.db, chief.id, atelier, true).await?;
    database::update_role(&state.db, chief.id, atelier, None, Some(true)).await?;
    database::add_role(&state.db, pending.id, atelier, false).await?;
    sqlx::query("INSERT INTO needs (day, atelier, quantity, nightly) VALUES (current_date + 1, $1, 1, false)")
        .bind(atelier).execute(&state.db).await?;
    let app = build_router(state);
    for path in [format!("/person/{}", chief.id), "/calendar".into()] {
        let response = request(&app, "GET", &path, Some(&cookie), serde_json::Value::Null).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(to_bytes(response.into_body(), 1_000_000).await?.to_vec())?;
        assert!(body.contains("reviewXss"));
        assert!(!body.contains(attack));
    }
    Ok(())
}

fn state(db: PgPool) -> AppState {
    let config = AppConfig {
        public_base_url: "https://station.example/powpow/".parse().unwrap(),
        listen_address: String::new(),
        database_url: String::new(),
        helloasso_client_id: String::new(),
        helloasso_client_secret: String::new(),
        helloasso_association_slug: String::new(),
        mailchimp_api_key: String::new(),
        mailchimp_server_prefix: String::new(),
        mailchimp_list_id: String::new(),
        mailchimp_from_name: String::new(),
        mailchimp_from_email: String::new(),
        smtp_host: String::new(),
        smtp_port: 587,
        smtp_user: String::new(),
        smtp_password: String::new(),
        smtp_from: "station@example.org".into(),
        gmail_client_id: String::new(),
        gmail_client_secret: String::new(),
        gmail_redirect_uri: String::new(),
        gmail_access_token: String::new(),
        gmail_refresh_token: String::new(),
        gmail_from: String::new(),
        mail_method: "smtp".into(),
        mail_destination_override: String::new(),
        cookie_secret: String::new(),
        sync_token: String::new(),
        backup_token: String::new(),
        huggingface_token: String::new(),
        rss_news_feed: String::new(),
        entity_name: "Test".into(),
        entity_email: "contact@example.org".into(),
    };
    AppState {
        db,
        config,
        cookie_key: Key::generate(),
        gmail_client: None,
        helloasso_client: HelloAssoClient::new(String::new(), String::new(), String::new()),
        mailchimp_client: MailchimpClient::new(
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ),
        sync_in_progress: std::sync::Arc::default(),
        mailchimp_sync_in_progress: std::sync::Arc::default(),
        mailchimp_sync_progress: std::sync::Arc::default(),
        mailchimp_sync_pending: std::sync::Arc::default(),
        mail_limits: std::sync::Arc::default(),
    }
}

async fn member(state: &AppState, email: &str) -> anyhow::Result<(models::Staff, String)> {
    let person: models::Staff = sqlx::query_as("INSERT INTO staff (first_name, last_name, email, comment) VALUES ('Test', 'Member', $1, '') RETURNING *")
        .bind(email).fetch_one(&state.db).await?;
    let cash: uuid::Uuid = sqlx::query_scalar("INSERT INTO cash (first_name, last_name, email, date, amount, is_membership) VALUES ('Test', 'Member', $1, current_date, 20, true) RETURNING id")
        .bind(email).fetch_one(&state.db).await?;
    sqlx::query("INSERT INTO payments (season, staff, cash_id) VALUES ($1, $2, $3)")
        .bind(get_current_season())
        .bind(person.id)
        .bind(cash)
        .execute(&state.db)
        .await?;
    let session = database::create_session(&state.db, person.id, None, None).await?;
    let jar = SignedCookieJar::new(state.cookie_key.clone()).add(
        axum_extra::extract::cookie::Cookie::new(auth::SESSION_COOKIE, session.to_string()),
    );
    let cookie = jar.into_response().headers()["set-cookie"]
        .to_str()?
        .to_string();
    Ok((person, cookie))
}

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    body: serde_json::Value,
) -> Response {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    app.clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn profile_privacy_and_privileged_identity_edits(pool: PgPool) -> anyhow::Result<()> {
    let state = state(pool);
    let (owner, owner_cookie) = member(&state, "private@example.org").await?;
    let (outsider, outsider_cookie) = member(&state, "outsider@example.org").await?;
    let (_, household_cookie) = member(&state, "private@example.org").await?;
    let qualification: i32 =
        sqlx::query_scalar("INSERT INTO qualifications (name) VALUES ('First aid') RETURNING id")
            .fetch_one(&state.db)
            .await?;
    let record = database::add_staff_qualif(
        &state.db,
        owner.id,
        qualification,
        chrono::Utc::now().date_naive(),
    )
    .await?;
    database::set_training_proof(&state.db, record.id, b"private proof", "image/png").await?;
    let app = build_router(state.clone());
    let profile = format!("/person/{}", owner.id);
    let response = request(
        &app,
        "GET",
        &profile,
        Some(&outsider_cookie),
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = String::from_utf8(to_bytes(response.into_body(), 1_000_000).await?.to_vec())?;
    assert!(!body.contains("private@example.org"));
    assert!(!body.contains("Historique des cotisations"));
    let proof = format!("/staff-qualif/{}/proof", record.id);
    assert_eq!(
        request(
            &app,
            "GET",
            &proof,
            Some(&outsider_cookie),
            serde_json::Value::Null
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    for cookie in [&owner_cookie, &household_cookie] {
        assert_eq!(
            request(&app, "GET", &proof, Some(cookie), serde_json::Value::Null)
                .await
                .status(),
            StatusCode::OK
        );
    }
    sqlx::query("UPDATE staff SET is_god = true, is_admin = true WHERE id = $1")
        .bind(owner.id)
        .execute(&state.db)
        .await?;
    sqlx::query("UPDATE staff SET is_admin = true WHERE id = $1")
        .bind(outsider.id)
        .execute(&state.db)
        .await?;
    let path = format!("/api/person/{}/contact", owner.id);
    let response = request(
        &app,
        "POST",
        &path,
        Some(&outsider_cookie),
        serde_json::json!({"email":"attacker@example.org"}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        database::get_staff_by_id(&state.db, owner.id)
            .await?
            .unwrap()
            .email,
        owner.email
    );
    Ok(())
}

#[sqlx::test]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL"]
async fn http_rejects_invalid_qualification_and_reports_mail_failure(
    pool: PgPool,
) -> anyhow::Result<()> {
    let state = state(pool);
    let (person, cookie) = member(&state, "member@example.org").await?;
    let app = build_router(state);
    assert_eq!(request(&app, "POST", "/api/my/staff-qualif", Some(&cookie), serde_json::json!({"staff_id":person.id,"qualification_id":1,"obtained_date":"+262142-12-31"})).await.status(), StatusCode::BAD_REQUEST);
    let form = serde_json::json!({"name":"Visitor", "email":"visitor@example.org", "subject":"Test", "message":"Test"});
    assert_eq!(
        request(&app, "POST", "/contact", None, form.clone())
            .await
            .status(),
        StatusCode::BAD_GATEWAY
    );
    for _ in 0..9 {
        request(&app, "POST", "/contact", None, form.clone()).await;
    }
    assert_eq!(
        request(&app, "POST", "/contact", None, form).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    Ok(())
}
