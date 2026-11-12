use super::*;
use axum::{
    Router,
    body::Body,
    http::{
        HeaderMap, StatusCode,
        header::{CONTENT_TYPE, REFERER},
    },
    response::{IntoResponse, Redirect},
    routing::get,
};
use std::{convert::Infallible, io::Cursor};

struct Server {
    url: reqwest::Url,
    task: tokio::task::JoinHandle<()>,
}

impl Server {
    async fn start(app: Router) -> Result<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("http://{}/", listener.local_addr()?).parse()?;
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Ok(Self { url, task })
    }

    fn fetcher(&self, path: &str) -> Result<Fetcher> {
        Fetcher::new(DirectConfig {
            url: self.url.join(path)?,
            period: Duration::from_mins(6),
            name: "pistes_nord-1".into(),
            title: Some("Vue sur les pistes".into()),
            insecure_tls: false,
        })
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[tokio::test]
async fn snapshots_use_content_detection_and_the_shared_persistent_archive() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("new/archive");
    let store = Store::open(
        archive.clone(),
        true,
        super::super::metadata::Settings::default(),
    )
    .await?;
    for (format, extension) in [
        (image::ImageFormat::Jpeg, "jpg"),
        (image::ImageFormat::Png, "png"),
        (image::ImageFormat::WebP, "webp"),
    ] {
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(4, 3).write_to(&mut encoded, format)?;
        let expected = encoded.into_inner();
        let body = expected.clone();
        let app = Router::new()
            .route(
                "/redirect",
                get(|| async { Redirect::temporary("/snapshot") }),
            )
            .route(
                "/snapshot",
                get(move |headers: HeaderMap| {
                    let body = body.clone();
                    async move {
                        assert_eq!(headers[CACHE_CONTROL], "no-cache");
                        assert!(!headers.contains_key(REFERER));
                        // Deliberately incorrect MIME type, with no URL extension.
                        ([(CONTENT_TYPE, "application/octet-stream")], body)
                    }
                }),
            );
        let server = Server::start(app).await?;
        let frame = server
            .fetcher("redirect?token=secret")?
            .fetch(&store)
            .await?;
        assert_eq!(frame.username, "pistes_nord-1");
        assert!(
            frame
                .filename
                .ends_with(&format!("_pistes_nord-1.{extension}"))
        );
        let archived = tokio::fs::read(store.path(&frame)).await?;
        assert_eq!(
            image::load_from_memory(&archived)?,
            image::load_from_memory(&expected)?
        );
        super::super::tests::assert_timestamp(&archived, &frame)?;
        assert_eq!(store.latest().unwrap().filename, frame.filename);
    }
    // FTPS and direct retrieval share the same store and index.
    store
        .save(crate::webcam::tests::png(), "camera".into(), "png".into())
        .await?;
    let reopened = Store::open(archive, false, super::super::metadata::Settings::default()).await?;
    assert_eq!(
        reopened
            .gallery(None, None, Some("pistes_nord-1"))
            .images
            .len(),
        3
    );
    assert_eq!(reopened.gallery(None, None, Some("camera")).images.len(), 1);
    Ok(())
}

#[tokio::test]
async fn invalid_and_failed_downloads_preserve_the_last_image_and_allow_recovery() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(
        directory.path().into(),
        true,
        super::super::metadata::Settings::default(),
    )
    .await?;
    let original = store
        .save(crate::webcam::tests::png(), "camera".into(), "png".into())
        .await?;
    let app = Router::new()
        .route("/good", get(|| async { crate::webcam::tests::png() }))
        .route(
            "/missing",
            get(|| async { (StatusCode::NOT_FOUND, crate::webcam::tests::png()) }),
        )
        .route(
            "/html",
            get(|| async { ([(CONTENT_TYPE, "image/jpeg")], "<html>Camera login</html>") }),
        )
        .route(
            "/truncated",
            get(|| async { crate::webcam::tests::png()[..33].to_vec() }),
        )
        .route(
            "/gif",
            get(|| async { b"GIF89a\x01\x00\x01\x00".as_slice() }),
        )
        .route(
            "/loop",
            get(|| async { Redirect::temporary("/loop?token=secret") }),
        )
        .route(
            "/interrupted",
            get(|| async {
                Body::from_stream(futures_util::stream::iter([
                    Ok(crate::webcam::tests::png()),
                    Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "interrupted download",
                    )),
                ]))
            }),
        );
    let server = Server::start(app).await?;
    let file_count = directory.path().read_dir()?.count();
    for path in ["missing", "html", "truncated", "gif", "loop", "interrupted"] {
        let error = server
            .fetcher(&format!("{path}?token=secret"))?
            .fetch(&store)
            .await
            .unwrap_err();
        assert!(!format!("{error:#}").contains("token=secret"));
        assert_eq!(
            store.latest().unwrap().filename,
            original.filename,
            "{path}"
        );
        assert_eq!(directory.path().read_dir()?.count(), file_count);
    }
    let next = server.fetcher("good")?.fetch(&store).await?;
    assert_ne!(next.filename, original.filename);
    assert_eq!(
        store
            .gallery(None, None, Some("pistes_nord-1"))
            .images
            .len(),
        1
    );
    assert_eq!(
        store.latest_for("camera").unwrap().filename,
        original.filename
    );
    Ok(())
}

#[tokio::test]
async fn size_limits_apply_to_content_length_and_chunked_responses() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(
        directory.path().into(),
        true,
        super::super::metadata::Settings::default(),
    )
    .await?;
    let app = Router::new()
        .route(
            "/length",
            get(|| async { vec![0u8; MAX_IMAGE_BYTES as usize + 1] }),
        )
        .route(
            "/chunked",
            get(|| async {
                Body::from_stream(futures_util::stream::iter(
                    (0..21).map(|_| Ok::<_, Infallible>(vec![0u8; 1024 * 1024])),
                ))
                .into_response()
            }),
        );
    let server = Server::start(app).await?;
    for path in ["length", "chunked"] {
        let error = server.fetcher(path)?.fetch(&store).await.unwrap_err();
        assert!(error.to_string().contains("20 MiB"), "{path}: {error:#}");
        assert!(store.latest().is_none());
        assert_eq!(directory.path().read_dir()?.count(), 0);
    }
    Ok(())
}
