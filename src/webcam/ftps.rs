use std::{
    future::{Future, ready},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use subtle::ConstantTimeEq;
use thirtyftp::{Backend, Entry, FsError, FtpPath, LoginContext, LoginError};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    net::TcpListener,
    sync::Semaphore,
};

use super::{MAX_IMAGE_BYTES, Store, config::FtpsConfig, image_format};

pub struct Server {
    listener: TcpListener,
    config: Arc<thirtyftp::Config>,
    backend: Arc<Uploads>,
}

impl Server {
    pub async fn bind(settings: &FtpsConfig, store: Store) -> Result<Self> {
        let certificates = CertificateDer::pem_file_iter(&settings.certificate)
            .context("Cannot read FTPS_TLS_CERT")?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let key = PrivateKeyDer::from_pem_file(&settings.private_key)
            .context("Cannot read FTPS_TLS_KEY")?;
        let tls = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::aws_lc_rs::default_provider(),
        ))
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(certificates, key)
        .context("Invalid FTPS certificate/key pair")?;
        let mut config = thirtyftp::Config::default();
        config.greeting = "PowPow webcam uploads".into();
        config.tls = Some(Arc::new(tls));
        config.require_tls = true;
        config.passive_ports = settings.passive_ports.clone();
        config.passive_host = settings.passive_host.clone();
        config.limits.max_transfer_time = Some(Duration::from_secs(120));
        let listener = TcpListener::bind(settings.listen_address)
            .await
            .context("Cannot bind FTPS_LISTEN_ADDRESS")?;
        tracing::info!(address = %listener.local_addr()?, "Webcam FTPS server listening");
        Ok(Self {
            listener,
            config: Arc::new(config),
            backend: Arc::new(Uploads {
                store,
                username: settings.username.clone(),
                password: settings.password.clone(),
            }),
        })
    }

    pub async fn run(self) -> Result<()> {
        let connections = Arc::new(Semaphore::new(8));
        loop {
            let (stream, peer) = self.listener.accept().await.context("FTPS accept failed")?;
            let Ok(permit) = connections.clone().try_acquire_owned() else {
                continue;
            };
            let config = self.config.clone();
            let backend = self.backend.clone();
            tokio::spawn(async move {
                let _permit = permit;
                thirtyftp::serve(stream, peer, config, backend, Arc::new(LogEvents)).await;
            });
        }
    }
}

struct LogEvents;

impl thirtyftp::Events for LogEvents {
    fn login_failed(&self, peer: SocketAddr, username: &str) {
        tracing::warn!(%peer, %username, "Webcam FTPS login failed");
    }
}

struct Uploads {
    store: Store,
    username: String,
    password: String,
}

impl Backend for Uploads {
    type User = ();

    fn login(
        &self,
        context: &LoginContext,
        username: &str,
        password: &str,
    ) -> impl Future<Output = Result<(), LoginError>> + Send {
        let credentials_match = username.as_bytes().ct_eq(self.username.as_bytes())
            & password.as_bytes().ct_eq(self.password.as_bytes());
        ready(if context.encrypted && bool::from(credentials_match) {
            Ok(())
        } else {
            Err(LoginError::Incorrect)
        })
    }

    // Camera-created date directories are virtual: uploads always enter our flat archive.
    fn metadata(
        &self,
        (): &(),
        path: &FtpPath,
    ) -> impl Future<Output = Result<Entry, FsError>> + Send {
        ready(Ok(Entry::directory(path.file_name().unwrap_or("/"), None)))
    }

    fn list(
        &self,
        (): &(),
        _: &FtpPath,
    ) -> impl Future<Output = Result<Vec<Entry>, FsError>> + Send {
        ready(Ok(Vec::new()))
    }

    fn read(
        &self,
        (): &(),
        _: &FtpPath,
        _: u64,
    ) -> impl Future<Output = Result<Box<dyn AsyncRead + Send + Unpin>, FsError>> + Send {
        ready(Err(FsError::PermissionDenied))
    }

    fn make_dir(&self, (): &(), _: &FtpPath) -> impl Future<Output = Result<(), FsError>> + Send {
        ready(Ok(()))
    }

    fn change_dir(&self, (): &(), _: &FtpPath) -> impl Future<Output = Result<(), FsError>> + Send {
        ready(Ok(()))
    }

    async fn write(
        &self,
        (): &(),
        path: &FtpPath,
        data: &mut (dyn AsyncRead + Send + Unpin),
    ) -> Result<u64, FsError> {
        let extension = path
            .file_name()
            .and_then(|name| name.rsplit_once('.'))
            .map(|(_, extension)| extension.to_ascii_lowercase())
            .ok_or(FsError::NameNotAllowed)?;
        if image_format(&extension).is_none() {
            return Err(FsError::NameNotAllowed);
        }
        // A failed, aborted, or truncated TLS transfer never reaches the filesystem.
        let mut bytes = Vec::new();
        data.take(MAX_IMAGE_BYTES + 1)
            .read_to_end(&mut bytes)
            .await?;
        let length = bytes.len() as u64;
        if length > MAX_IMAGE_BYTES {
            return Err(FsError::QuotaExceeded);
        }
        self.store
            .save(bytes, self.username.clone(), extension)
            .await
            .map_err(|error| {
                tracing::warn!(%error, "Webcam upload rejected");
                FsError::Local(error.to_string())
            })?;
        Ok(length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io,
        pin::Pin,
        task::{Context as TaskContext, Poll},
    };
    use tokio::io::ReadBuf;

    struct InterruptedUpload;

    impl AsyncRead for InterruptedUpload {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut TaskContext<'_>,
            _: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Poll::Ready(Err(io::ErrorKind::UnexpectedEof.into()))
        }
    }

    #[tokio::test]
    async fn uploads_require_credentials_tls_and_a_complete_image() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = Store::open(
            directory.path().into(),
            true,
            super::super::metadata::Settings::default(),
        )
        .await?;
        let uploads = Uploads {
            store: store.clone(),
            username: "camera".into(),
            password: "secret".into(),
        };
        let address = "127.0.0.1:2121".parse()?;
        let plain = LoginContext::new(address, address, false);
        let secure = LoginContext::new(address, address, true);
        assert!(uploads.login(&plain, "camera", "secret").await.is_err());
        assert!(uploads.login(&secure, "camera", "wrong").await.is_err());
        assert!(uploads.login(&secure, "other", "secret").await.is_err());
        uploads.login(&secure, "camera", "secret").await?;
        let path = FtpPath::resolve(&FtpPath::root(), "/camera/2026/11/image.PNG")?;
        assert!(
            uploads
                .write(&(), &path, &mut InterruptedUpload)
                .await
                .is_err()
        );
        assert!(store.latest().is_none());
        let bytes = super::super::tests::png();
        for _ in 0..2 {
            assert_eq!(
                uploads.write(&(), &path, &mut bytes.as_slice()).await?,
                bytes.len() as u64
            );
        }
        assert_eq!(store.gallery(None, None, None).images.len(), 2);
        let frame = store.latest().unwrap();
        super::super::tests::assert_timestamp(&std::fs::read(store.path(&frame))?, &frame)?;
        let file_count = directory.path().read_dir()?.count();
        assert_eq!(store.latest().unwrap().username, "camera");
        assert!(uploads.read(&(), &path, 0).await.is_err());
        let rejected = FtpPath::resolve(&FtpPath::root(), "/video.mp4")?;
        assert!(
            uploads
                .write(&(), &rejected, &mut bytes.as_slice())
                .await
                .is_err()
        );
        let mut oversized = tokio::io::repeat(0).take(MAX_IMAGE_BYTES + 1);
        assert_eq!(
            uploads.write(&(), &path, &mut oversized).await,
            Err(FsError::QuotaExceeded)
        );
        assert_eq!(directory.path().read_dir()?.count(), file_count);
        Ok(())
    }
}
