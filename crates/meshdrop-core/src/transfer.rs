//! HTTPS invitations and bounded streaming, with decisions delegated to the frontend.
use crate::{
    config::Config,
    identity::{Identity, TrustStore, fingerprint, short_code},
    sink::Sink,
    tls::Verifier,
};
use anyhow::{Context, Result, bail};
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Extension, Path, State},
    http::StatusCode,
    routing::{get, post},
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::AsyncReadExt,
    net::TcpListener,
    sync::{mpsc, oneshot},
};
use tokio_util::{io::ReaderStream, sync::CancellationToken};
use uuid::Uuid;
/// Every wire object carries a protocol version to reject incompatible clients.
pub const VERSION: u32 = 1;
/// Discovery metadata is descriptive; only the TLS certificate establishes identity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Info {
    pub version: u32,
    pub name: String,
    pub platform: String,
    pub fingerprint: String,
}
/// A manifest is authorized before any file content is transmitted.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileOffer {
    pub name: String,
    pub size: u64,
    pub sha256: String,
}
#[derive(Serialize, Deserialize)]
struct Offer {
    version: u32,
    files: Vec<FileOffer>,
}
#[derive(Serialize, Deserialize)]
struct Pair {
    version: u32,
    address: String,
}
/// Acceptance is explicit; dropping a decision sender means rejection.
#[derive(Clone, Copy, Debug)]
pub enum Decision {
    Accept,
    Reject,
}
/// Frontends receive invitations through a bounded channel and own the decision.
#[derive(Debug)]
pub struct Invitation {
    pub peer: String,
    pub kind: InvitationKind,
    pub decision: oneshot::Sender<Decision>,
    pub cancel: CancellationToken,
}
/// Pairing and transfers share confirmation plumbing but remain distinct actions.
#[derive(Debug)]
pub enum InvitationKind {
    Pair { code: String },
    Files(Vec<FileOffer>),
}
/// Bounded progress events can be rendered by any frontend without polling files.
#[derive(Clone, Debug)]
pub struct Progress {
    pub transfer: String,
    pub name: String,
    pub bytes: u64,
    pub total: u64,
}
#[derive(Clone)]
struct Active {
    owner: String,
    files: Vec<Option<FileOffer>>,
    cancel: CancellationToken,
}
struct Inner {
    trust: TrustStore,
    active: HashMap<String, Active>,
}
#[derive(Clone)]
struct ServerState {
    config: Config,
    directory: PathBuf,
    info: Info,
    invitations: mpsc::Sender<Invitation>,
    progress: mpsc::Sender<Progress>,
    inner: Arc<Mutex<Inner>>,
}
type ApiResult<T> = std::result::Result<Json<T>, (StatusCode, String)>;
fn api_error(error: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, error.to_string())
}
fn version(v: u32) -> Result<()> {
    if v != VERSION {
        bail!("unsupported protocol version {v}");
    }
    Ok(())
}

/// Runs until cancellation, then terminates active connections and removes partial
/// downloads. Network policy is enforced before the TLS handshake.
pub async fn serve(
    listener: TcpListener,
    config: Config,
    directory: PathBuf,
    identity: Identity,
    invitations: mpsc::Sender<Invitation>,
    progress: mpsc::Sender<Progress>,
    shutdown: CancellationToken,
) -> Result<()> {
    std::fs::create_dir_all(&config.receive_directory)?;
    let tls = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_client_cert_verifier(Arc::new(Verifier(None)))
    .with_single_cert(vec![identity.certificate()], identity.key())?;
    let state = ServerState {
        info: Info {
            version: VERSION,
            name: config.name.clone(),
            platform: std::env::consts::OS.into(),
            fingerprint: identity.fingerprint(),
        },
        config,
        inner: Arc::new(Mutex::new(Inner {
            trust: TrustStore::load(&directory.join("trust.json"))?,
            active: HashMap::new(),
        })),
        directory,
        invitations,
        progress,
    };
    let router = Router::new()
        .route("/info", get(info))
        .route("/peers", get(peers))
        .route("/pair", post(pair))
        .route("/offer", post(offer))
        .route("/upload/{id}/{index}", post(upload))
        .route("/cancel/{id}", post(cancel))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .with_state(state.clone());
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(tls));
    let mut tasks = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
         _ = shutdown.cancelled() => break,
         result = tasks.join_next(), if !tasks.is_empty() => { result.context("connection task missing")???; },
         accepted = listener.accept() => {
          let (stream, address) = accepted?;
          if !state.config.allows(address.ip()) { eprintln!("source network denied: {}", address.ip()); continue; }
          let acceptor = acceptor.clone(); let router = router.clone(); let stop = shutdown.clone();
          tasks.spawn(async move {
           let connection = async {
            let stream = tokio::time::timeout(Duration::from_secs(10), acceptor.accept(stream)).await??;
            let cert = stream.get_ref().1.peer_certificates().and_then(|c| c.first()).context("missing client certificate")?;
            let peer = fingerprint(cert.as_ref());
            let service = hyper_util::service::TowerToHyperService::new(router.layer(Extension(peer)));
            hyper::server::conn::http1::Builder::new().serve_connection(hyper_util::rt::TokioIo::new(stream), service).await?;
            Ok::<_, anyhow::Error>(())
           };
           tokio::select! { result = connection => { if let Err(error) = result { eprintln!("connection failed: {error:#}"); } }, _ = stop.cancelled() => {} }
           Ok::<_, anyhow::Error>(())
          });
         }
        }
    }
    while let Some(task) = tasks.join_next().await {
        task??;
    }
    Ok(())
}
async fn info(State(s): State<ServerState>) -> Json<Info> {
    Json(s.info)
}
async fn peers(
    State(s): State<ServerState>,
    Extension(peer): Extension<String>,
) -> ApiResult<HashMap<String, String>> {
    let inner = s.inner.lock().map_err(api_error)?;
    authorized(&inner, &peer).map_err(api_error)?;
    Ok(Json(inner.trust.0.clone().into_iter().collect()))
}
fn authorized(inner: &Inner, peer: &str) -> Result<()> {
    if !inner.trust.0.values().any(|p| p == peer) {
        bail!("unpaired device");
    }
    Ok(())
}
async fn decide(
    s: &ServerState,
    peer: String,
    kind: InvitationKind,
    cancel: CancellationToken,
) -> Result<()> {
    let (tx, rx) = oneshot::channel();
    let timeout = Duration::from_secs(s.config.timeout_seconds);
    let result = async {
        s.invitations
            .send(Invitation {
                peer,
                kind,
                decision: tx,
                cancel: cancel.clone(),
            })
            .await
            .context("frontend disconnected")?;
        match rx.await {
            Ok(Decision::Accept) => Ok(()),
            _ => bail!("invitation rejected"),
        }
    };
    tokio::select! { _ = cancel.cancelled() => bail!("transfer cancelled"), result = tokio::time::timeout(timeout, result) => result.context("invitation timed out")? }
}
async fn pair(
    State(s): State<ServerState>,
    Extension(peer): Extension<String>,
    Json(request): Json<Pair>,
) -> ApiResult<Info> {
    version(request.version).map_err(api_error)?;
    let code = short_code(&s.info.fingerprint, &peer);
    decide(
        &s,
        peer.clone(),
        InvitationKind::Pair { code },
        CancellationToken::new(),
    )
    .await
    .map_err(api_error)?;
    s.inner
        .lock()
        .map_err(api_error)?
        .trust
        .remember(&s.directory.join("trust.json"), &request.address, &peer)
        .map_err(api_error)?;
    Ok(Json(s.info))
}
async fn offer(
    State(s): State<ServerState>,
    Extension(peer): Extension<String>,
    Json(request): Json<Offer>,
) -> ApiResult<String> {
    version(request.version).map_err(api_error)?;
    {
        let inner = s.inner.lock().map_err(api_error)?;
        authorized(&inner, &peer).map_err(api_error)?;
    }
    if request.files.is_empty() || request.files.len() > 128 {
        return Err(api_error("offer must contain 1..128 files"));
    }
    for file in &request.files {
        crate::sink::sanitize(&file.name).map_err(api_error)?;
        if file.sha256.len() != 64 || hex::decode(&file.sha256).is_err() {
            return Err(api_error("invalid SHA-256"));
        }
    }
    let cancel = CancellationToken::new();
    decide(
        &s,
        peer.clone(),
        InvitationKind::Files(request.files.clone()),
        cancel.clone(),
    )
    .await
    .map_err(api_error)?;
    let id = Uuid::new_v4().to_string();
    s.inner.lock().map_err(api_error)?.active.insert(
        id.clone(),
        Active {
            owner: peer,
            files: request.files.into_iter().map(Some).collect(),
            cancel: cancel.clone(),
        },
    );
    let state = s.clone();
    let expired = id.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(state.config.timeout_seconds)).await;
        match state.inner.lock() {
            Ok(mut inner) => {
                if let Some(active) = inner.active.remove(&expired) {
                    active.cancel.cancel();
                }
            }
            Err(error) => eprintln!("transfer expiry: {error}"),
        }
    });
    Ok(Json(id))
}
async fn cancel(
    State(s): State<ServerState>,
    Extension(peer): Extension<String>,
    Path(id): Path<String>,
) -> ApiResult<bool> {
    let mut inner = s.inner.lock().map_err(api_error)?;
    if inner
        .active
        .get(&id)
        .is_some_and(|active| active.owner == peer)
    {
        if let Some(active) = inner.active.remove(&id) {
            active.cancel.cancel();
        }
        return Ok(Json(true));
    }
    Err(api_error("unknown transfer"))
}
async fn upload(
    State(s): State<ServerState>,
    Extension(peer): Extension<String>,
    Path((id, index)): Path<(String, usize)>,
    body: Body,
) -> ApiResult<String> {
    let (file, cancel) = {
        let mut inner = s.inner.lock().map_err(api_error)?;
        let active = inner
            .active
            .get_mut(&id)
            .filter(|a| a.owner == peer)
            .ok_or_else(|| api_error("unknown transfer"))?;
        let file = active
            .files
            .get_mut(index)
            .and_then(Option::take)
            .ok_or_else(|| api_error("unknown or already uploaded file"))?;
        (file, active.cancel.clone())
    };
    let receive = async {
        let mut sink = Sink::new(&s.config.receive_directory, &file.name)?;
        let mut stream = body.into_data_stream();
        let mut bytes = 0u64;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            bytes = bytes
                .checked_add(chunk.len() as u64)
                .context("size overflow")?;
            if bytes > file.size {
                bail!("upload exceeds offered size");
            }
            sink.write(&chunk)?;
            s.progress
                .send(Progress {
                    transfer: id.clone(),
                    name: file.name.clone(),
                    bytes,
                    total: file.size,
                })
                .await
                .context("progress consumer disconnected")?;
        }
        let path = sink.finish(file.size, &file.sha256)?;
        Ok::<_, anyhow::Error>(path.display().to_string())
    };
    tokio::select! { _ = cancel.cancelled() => Err(api_error("transfer cancelled")), result = tokio::time::timeout(Duration::from_secs(s.config.timeout_seconds), receive) => Ok(Json(result.map_err(api_error)?.map_err(api_error)?)) }
}

/// A client uses mutual TLS and pins on every connection. Bootstrap clients may
/// fetch metadata only; callers must compare QR pins or confirm the short code.
pub struct Client {
    http: reqwest::Client,
    address: String,
}
impl Client {
    /// Builds a bounded streaming client. A supplied pin is checked during TLS,
    /// including reconnects, redirects are disabled to avoid credential leakage.
    pub fn new(
        address: &str,
        identity: &Identity,
        pin: Option<String>,
        timeout: Duration,
    ) -> Result<Self> {
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Verifier(pin)))
        .with_client_auth_cert(vec![identity.certificate()], identity.key())?;
        let http = reqwest::Client::builder()
            .use_preconfigured_tls(tls)
            .tls_info(true)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .timeout(timeout)
            .build()?;
        Ok(Self {
            http,
            address: format!(
                "https://{}",
                address.trim_start_matches("https://").trim_end_matches('/')
            ),
        })
    }
    async fn checked(response: reqwest::Response) -> Result<reqwest::Response> {
        if !response.status().is_success() {
            bail!("peer refused: {}", response.text().await?);
        }
        Ok(response)
    }
    /// Fetches metadata and checks that its identity matches the actual TLS certificate.
    pub async fn info(&self) -> Result<Info> {
        let response = Self::checked(
            self.http
                .get(format!("{}/info", self.address))
                .send()
                .await?,
        )
        .await?;
        let pin = fingerprint(
            response
                .extensions()
                .get::<reqwest::tls::TlsInfo>()
                .and_then(|t| t.peer_certificate())
                .context("missing TLS peer certificate")?,
        );
        let info: Info = response.json().await?;
        version(info.version)?;
        if info.fingerprint != pin {
            bail!("fingerprint mismatch in info");
        }
        Ok(info)
    }
    /// Requests reciprocal confirmation. Trust is stored by the caller only on success.
    pub async fn pair(&self, address: &str) -> Result<Info> {
        Ok(Self::checked(
            self.http
                .post(format!("{}/pair", self.address))
                .json(&Pair {
                    version: VERSION,
                    address: address.into(),
                })
                .send()
                .await?,
        )
        .await?
        .json()
        .await?)
    }
    /// Asks a paired peer for known addresses; its advertised pins still require
    /// user confirmation locally and never implicitly grant trust.
    pub async fn peers(&self) -> Result<HashMap<String, String>> {
        Ok(Self::checked(
            self.http
                .get(format!("{}/peers", self.address))
                .send()
                .await?,
        )
        .await?
        .json()
        .await?)
    }
    /// Hashes with a fixed-size buffer so manifests never require loading files.
    pub async fn manifest(paths: &[PathBuf]) -> Result<Vec<FileOffer>> {
        let mut offers = Vec::new();
        for path in paths {
            let mut file = tokio::fs::File::open(path).await?;
            let mut hash = Sha256::new();
            let mut buffer = vec![0; 65536];
            let mut size = 0;
            loop {
                let count = file.read(&mut buffer).await?;
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
                size += count as u64;
            }
            offers.push(FileOffer {
                name: path
                    .file_name()
                    .context("file has no name")?
                    .to_string_lossy()
                    .into(),
                size,
                sha256: hex::encode(hash.finalize()),
            });
        }
        Ok(offers)
    }
    /// Sends one invitation then uploads files sequentially with a 64-KiB read buffer.
    /// Local cancellation calls the peer's cancellation API before returning.
    pub async fn send(
        &self,
        paths: &[PathBuf],
        progress: mpsc::Sender<Progress>,
        cancel: CancellationToken,
    ) -> Result<()> {
        let files = Self::manifest(paths).await?;
        let request = self
            .http
            .post(format!("{}/offer", self.address))
            .json(&Offer {
                version: VERSION,
                files: files.clone(),
            })
            .send();
        let response = tokio::select! { _ = cancel.cancelled() => bail!("transfer cancelled"), result = request => result? };
        let id: String = Self::checked(response).await?.json().await?;
        let sending = self.upload_files(&id, paths, files, progress);
        tokio::select! { result = sending => result, _ = cancel.cancelled() => {
         Self::checked(self.http.post(format!("{}/cancel/{id}",self.address)).send().await?).await?;
         bail!("transfer cancelled")
        } }
    }
    async fn upload_files(
        &self,
        id: &str,
        paths: &[PathBuf],
        files: Vec<FileOffer>,
        progress: mpsc::Sender<Progress>,
    ) -> Result<()> {
        for (index, (path, offer)) in paths.iter().zip(files).enumerate() {
            let file = tokio::fs::File::open(path).await?;
            let tx = progress.clone();
            let transfer = id.to_owned();
            let mut bytes = 0;
            let stream = ReaderStream::with_capacity(file, 65536).then(move |chunk| {
                let tx = tx.clone();
                let transfer = transfer.clone();
                let name = offer.name.clone();
                if let Ok(chunk) = &chunk {
                    bytes += chunk.len() as u64;
                }
                let bytes = bytes;
                async move {
                    tx.send(Progress {
                        transfer,
                        name,
                        bytes,
                        total: offer.size,
                    })
                    .await
                    .map_err(std::io::Error::other)?;
                    chunk
                }
            });
            Self::checked(
                self.http
                    .post(format!("{}/upload/{id}/{index}", self.address))
                    .body(reqwest::Body::wrap_stream(stream))
                    .send()
                    .await?,
            )
            .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Harness {
        _directory: tempfile::TempDir,
        receiver: PathBuf,
        source: PathBuf,
        client: Arc<Client>,
        invitations: mpsc::Receiver<Invitation>,
        progress: mpsc::Receiver<Progress>,
        stop: CancellationToken,
        server: tokio::task::JoinHandle<Result<()>>,
    }
    impl Harness {
        async fn new(timeout_seconds: u64) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let receiver = dir.path().join("received");
            let state = dir.path().join("server");
            let server_id = Identity::load(&state).unwrap();
            let client_id = Identity::load(&dir.path().join("client")).unwrap();
            TrustStore::default()
                .remember(
                    &state.join("trust.json"),
                    "client",
                    &client_id.fingerprint(),
                )
                .unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap().to_string();
            let client = Arc::new(
                Client::new(
                    &address,
                    &client_id,
                    Some(server_id.fingerprint()),
                    Duration::from_secs(10),
                )
                .unwrap(),
            );
            let (tx, invitations) = mpsc::channel(16);
            let (progress_tx, progress) = mpsc::channel(16);
            let stop = CancellationToken::new();
            let source = dir.path().join("source.txt");
            std::fs::write(&source, b"hello").unwrap();
            let server = tokio::spawn(serve(
                listener,
                Config {
                    receive_directory: receiver.clone(),
                    timeout_seconds,
                    ..Config::default()
                },
                state,
                server_id,
                tx,
                progress_tx,
                stop.clone(),
            ));
            Self {
                _directory: dir,
                receiver,
                source,
                client,
                invitations,
                progress,
                stop,
                server,
            }
        }
        fn sending(&self, token: CancellationToken) -> tokio::task::JoinHandle<Result<()>> {
            let client = self.client.clone();
            let source = self.source.clone();
            tokio::spawn(async move {
                let (tx, mut rx) = mpsc::channel(16);
                let consumer = tokio::spawn(async move { while rx.recv().await.is_some() {} });
                let result = client.send(&[source], tx, token).await;
                consumer.await.unwrap();
                result
            })
        }
        async fn finish(self) {
            self.stop.cancel();
            self.server.await.unwrap().unwrap();
        }
    }
    #[tokio::test]
    async fn invitation_accepts_and_emits_receiver_progress() {
        let mut h = Harness::new(10).await;
        let sending = h.sending(CancellationToken::new());
        let invitation = h.invitations.recv().await.unwrap();
        assert!(matches!(invitation.kind, InvitationKind::Files(_)));
        invitation.decision.send(Decision::Accept).unwrap();
        sending.await.unwrap().unwrap();
        assert_eq!(h.progress.recv().await.unwrap().bytes, 5);
        assert_eq!(
            std::fs::read(h.receiver.join("source.txt")).unwrap(),
            b"hello"
        );
        h.finish().await;
    }
    #[tokio::test]
    async fn rejection_transmits_no_file() {
        let mut h = Harness::new(10).await;
        let sending = h.sending(CancellationToken::new());
        h.invitations
            .recv()
            .await
            .unwrap()
            .decision
            .send(Decision::Reject)
            .unwrap();
        assert!(
            sending
                .await
                .unwrap()
                .unwrap_err()
                .to_string()
                .contains("rejected")
        );
        assert_eq!(std::fs::read_dir(&h.receiver).unwrap().count(), 0);
        h.finish().await;
    }
    #[tokio::test]
    async fn unanswered_invitation_times_out() {
        let mut h = Harness::new(1).await;
        let sending = h.sending(CancellationToken::new());
        let _invitation = h.invitations.recv().await.unwrap();
        assert!(
            sending
                .await
                .unwrap()
                .unwrap_err()
                .to_string()
                .contains("timed out")
        );
        h.finish().await;
    }
    #[tokio::test]
    async fn receiver_cancels_before_upload() {
        let mut h = Harness::new(10).await;
        let sending = h.sending(CancellationToken::new());
        let invitation = h.invitations.recv().await.unwrap();
        invitation.cancel.cancel();
        invitation.decision.send(Decision::Accept).unwrap();
        assert!(sending.await.unwrap().is_err());
        assert_eq!(std::fs::read_dir(&h.receiver).unwrap().count(), 0);
        h.finish().await;
    }
    #[tokio::test]
    async fn sender_cancels_pending_invitation() {
        let mut h = Harness::new(10).await;
        let token = CancellationToken::new();
        let sending = h.sending(token.clone());
        let _invitation = h.invitations.recv().await.unwrap();
        token.cancel();
        assert!(
            sending
                .await
                .unwrap()
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
        assert_eq!(std::fs::read_dir(&h.receiver).unwrap().count(), 0);
        h.finish().await;
    }
    #[tokio::test]
    async fn changed_certificate_fails_tls() {
        let h = Harness::new(10).await;
        let identity = Identity::load(&h._directory.path().join("other")).unwrap();
        let client = Client::new(
            h.client.address.trim_start_matches("https://"),
            &identity,
            Some("wrong".into()),
            Duration::from_secs(2),
        )
        .unwrap();
        assert!(client.info().await.is_err());
        h.finish().await;
    }
    #[tokio::test]
    async fn pairing_requires_receiver_confirmation() {
        let mut h = Harness::new(10).await;
        let client = h.client.clone();
        let request = tokio::spawn(async move { client.pair("127.0.0.1:1234").await });
        let invitation = h.invitations.recv().await.unwrap();
        assert!(matches!(invitation.kind, InvitationKind::Pair { .. }));
        invitation.decision.send(Decision::Accept).unwrap();
        assert_eq!(request.await.unwrap().unwrap().version, VERSION);
        h.finish().await;
    }
    #[tokio::test]
    async fn one_invitation_authorizes_multiple_files() {
        let mut h = Harness::new(10).await;
        let second = h._directory.path().join("second");
        std::fs::write(&second, b"world").unwrap();
        let paths = vec![h.source.clone(), second];
        let client = h.client.clone();
        let (tx, mut rx) = mpsc::channel(16);
        let request =
            tokio::spawn(async move { client.send(&paths, tx, CancellationToken::new()).await });
        let invitation = h.invitations.recv().await.unwrap();
        invitation.decision.send(Decision::Accept).unwrap();
        request.await.unwrap().unwrap();
        assert_eq!(rx.recv().await.unwrap().bytes, 5);
        assert_eq!(std::fs::read(h.receiver.join("second")).unwrap(), b"world");
        h.finish().await;
    }
    async fn raw_offer(h: &mut Harness, size: u64, sha256: String) -> String {
        let client = h.client.clone();
        let response = tokio::spawn(async move {
            client
                .http
                .post(format!("{}/offer", client.address))
                .json(&Offer {
                    version: VERSION,
                    files: vec![FileOffer {
                        name: "raw".into(),
                        size,
                        sha256,
                    }],
                })
                .send()
                .await
                .unwrap()
                .json::<String>()
                .await
                .unwrap()
        });
        h.invitations
            .recv()
            .await
            .unwrap()
            .decision
            .send(Decision::Accept)
            .unwrap();
        response.await.unwrap()
    }
    #[tokio::test]
    async fn corrupt_stream_is_deleted() {
        let mut h = Harness::new(10).await;
        let id = raw_offer(&mut h, 3, "00".repeat(32)).await;
        let response = h
            .client
            .http
            .post(format!("{}/upload/{id}/0", h.client.address))
            .body("bad")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(std::fs::read_dir(&h.receiver).unwrap().count(), 0);
        h.finish().await;
    }
    #[tokio::test]
    async fn disconnect_during_stream_removes_partial_file() {
        let mut h = Harness::new(1).await;
        let id = raw_offer(&mut h, 100, "00".repeat(32)).await;
        let (tx, rx) = mpsc::channel::<Result<bytes::Bytes, std::io::Error>>(2);
        let body = futures_util::stream::unfold(rx, |mut rx| async move {
            rx.recv().await.map(|item| (item, rx))
        });
        tx.send(Ok(bytes::Bytes::from_static(b"partial")))
            .await
            .unwrap();
        let client = h.client.clone();
        let request = tokio::spawn(async move {
            client
                .http
                .post(format!("{}/upload/{id}/0", client.address))
                .body(reqwest::Body::wrap_stream(body))
                .send()
                .await
        });
        assert_eq!(h.progress.recv().await.unwrap().bytes, 7);
        request.abort();
        drop(tx);
        tokio::time::sleep(Duration::from_millis(1200)).await;
        assert_eq!(std::fs::read_dir(&h.receiver).unwrap().count(), 0);
        h.finish().await;
    }
    #[tokio::test]
    async fn receiver_cancels_active_stream_and_cleans_partial() {
        let mut h = Harness::new(10).await;
        let client = h.client.clone();
        let offer = tokio::spawn(async move {
            client
                .http
                .post(format!("{}/offer", client.address))
                .json(&Offer {
                    version: VERSION,
                    files: vec![FileOffer {
                        name: "partial".into(),
                        size: 100,
                        sha256: "00".repeat(32),
                    }],
                })
                .send()
                .await
                .unwrap()
                .json::<String>()
                .await
                .unwrap()
        });
        let invitation = h.invitations.recv().await.unwrap();
        invitation.decision.send(Decision::Accept).unwrap();
        let id = offer.await.unwrap();
        let (tx, rx) = mpsc::channel::<Result<bytes::Bytes, std::io::Error>>(2);
        tx.send(Ok(bytes::Bytes::from_static(b"partial")))
            .await
            .unwrap();
        let body = futures_util::stream::unfold(rx, |mut rx| async move {
            rx.recv().await.map(|item| (item, rx))
        });
        let client = h.client.clone();
        let upload = tokio::spawn(async move {
            client
                .http
                .post(format!("{}/upload/{id}/0", client.address))
                .body(reqwest::Body::wrap_stream(body))
                .send()
                .await
        });
        h.progress.recv().await.unwrap();
        invitation.cancel.cancel();
        assert_eq!(
            upload.await.unwrap().unwrap().status(),
            StatusCode::BAD_REQUEST
        );
        drop(tx);
        assert_eq!(std::fs::read_dir(&h.receiver).unwrap().count(), 0);
        h.finish().await;
    }
}
