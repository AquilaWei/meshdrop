//! One actor owns frontend state; bounded channels serialize decisions and updates.
use anyhow::{Context, Result, bail};
use meshdrop_core::{
    config::Config,
    discovery::Mdns,
    identity::Identity,
    transfer::{self, Invitation, Progress},
};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::{mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;

/// Native shell effects are separate from state so tests do not require a webview.
#[derive(Debug)]
pub enum Notice {
    /// Invitation content is descriptive and must never be interpreted as markup.
    Invitation(String),
}
struct Request {
    operation: String,
    payload: Value,
    reply: oneshot::Sender<Result<Value, String>>,
}
/// A cloneable handle exposes commands without sharing mutable UI state.
#[derive(Clone)]
pub struct Controller {
    requests: mpsc::Sender<Request>,
    stop: CancellationToken,
    done: watch::Receiver<bool>,
}
impl Controller {
    /// Binds before returning so port conflicts fail startup rather than losing reception.
    /// Directory, identity, TOML, multicast and listener failures are returned to the shell.
    pub async fn start(directory: PathBuf, notices: mpsc::Sender<Notice>) -> Result<Self> {
        let folder = directory.clone();
        let (config, identity) = tokio::task::spawn_blocking(move || {
            let config = Config::load(&folder.join("config.toml"))?;
            let identity = Arc::new(Identity::load(&folder)?);
            Ok::<_, anyhow::Error>((config, identity))
        })
        .await??;
        let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.port)).await?;
        let port = listener.local_addr()?.port();
        let mdns = if config.providers.iter().any(|p| p == "mdns") {
            let mdns = Mdns::new(Duration::from_secs(2), identity.clone())?;
            mdns.advertise(&config.name, port)?;
            Some(mdns)
        } else {
            None
        };
        let (requests, rx) = mpsc::channel(32);
        let (invitation_tx, invitations) = mpsc::channel(16);
        let (progress_tx, progress) = mpsc::channel(16);
        let stop = CancellationToken::new();
        let (done_tx, done) = watch::channel(false);
        let server = tokio::spawn(transfer::serve(
            listener,
            config.clone(),
            directory.clone(),
            identity.clone(),
            invitation_tx,
            progress_tx,
            stop.clone(),
        ));
        let actor = Actor {
            session: Session {
                directory,
                config,
                identity,
                port,
            },
            stop: stop.clone(),
            notices,
            view: json!({"devices":[],"problems":[],"invitations":[],"transfers":[],"error":null}),
            pending: Default::default(),
            manual: Vec::new(),
        };
        tokio::spawn(async move {
            actor.run(rx, invitations, progress).await;
            match server.await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => eprintln!("desktop server: {error:#}"),
                Err(error) => eprintln!("desktop server task: {error}"),
            }
            if let Some(mdns) = mdns
                && let Err(error) = mdns.shutdown()
            {
                eprintln!("desktop mDNS shutdown: {error:#}");
            }
            let _ = done_tx.send(true);
        });
        Ok(Self {
            requests,
            stop,
            done,
        })
    }
    /// Sends a command to the actor; malformed or stale actions return displayable errors.
    pub async fn call(&self, operation: &str, payload: Value) -> Result<Value, String> {
        let (reply, result) = oneshot::channel();
        self.requests
            .send(Request {
                operation: operation.into(),
                payload,
                reply,
            })
            .await
            .map_err(|_| "application stopped".to_owned())?;
        result.await.map_err(|_| "application stopped".to_owned())?
    }
    /// Cancels network work and waits for temporary-file cleanup and mDNS withdrawal.
    pub async fn shutdown(&self) {
        self.stop.cancel();
        let mut done = self.done.clone();
        let _ = done.wait_for(|finished| *finished).await;
    }
}
#[derive(Clone)]
struct Session {
    directory: PathBuf,
    config: Config,
    identity: Arc<Identity>,
    port: u16,
}
struct Actor {
    session: Session,
    stop: CancellationToken,
    notices: mpsc::Sender<Notice>,
    view: Value,
    pending: std::collections::HashMap<String, Invitation>,
    manual: Vec<String>,
}
struct Job {
    operation: String,
    result: Result<Value>,
    reply: oneshot::Sender<Result<Value, String>>,
}
impl Actor {
    async fn run(
        mut self,
        mut requests: mpsc::Receiver<Request>,
        mut invitations: mpsc::Receiver<Invitation>,
        mut progress: mpsc::Receiver<Progress>,
    ) {
        let mut jobs = tokio::task::JoinSet::new();
        let mut expiry = tokio::time::interval(Duration::from_millis(250));
        loop {
            tokio::select! {
                _ = self.stop.cancelled() => break,
                Some(request) = requests.recv() => self.command(request, &mut jobs),
                Some(invitation) = invitations.recv() => self.invite(invitation).await,
                Some(_event) = progress.recv() => {},
                Some(result) = jobs.join_next() => match result {
                    Ok(job) => {
                        if let Ok(value) = &job.result && job.operation == "refresh" {
                            self.view["devices"] = value["devices"].clone();
                            self.view["problems"] = value["problems"].clone();
                        }
                        let _ = job.reply.send(job.result.map_err(|e| format!("{e:#}")));
                    },
                    Err(error) => self.view["error"] = json!(format!("desktop worker: {error}")),
                },
                _ = expiry.tick() => {
                    self.pending.retain(|_, invitation| !invitation.decision.is_closed());
                    self.view["invitations"].as_array_mut().expect("array").retain(|item| self.pending.contains_key(item["id"].as_str().unwrap_or_default()));
                },
            }
        }
        self.stop.cancel();
        self.pending.clear();
        jobs.abort_all();
        while jobs.join_next().await.is_some() {}
    }
    async fn invite(&mut self, invitation: Invitation) {
        let id = invitation.transfer.clone();
        let (kind, detail) = match &invitation.kind {
            transfer::InvitationKind::Pair { code } => ("pair", json!({"code": code})),
            transfer::InvitationKind::Files(files) => ("files", json!({"files": files})),
        };
        self.view["invitations"].as_array_mut().expect("array").push(json!({"id": id, "kind": kind, "peer": invitation.peer, "name": invitation.peer_name, "detail": detail}));
        if let Err(error) = self
            .notices
            .send(Notice::Invitation(format!(
                "{} requests {kind}",
                invitation.peer_name
            )))
            .await
        {
            self.view["error"] = json!(format!("notification channel: {error}"));
        }
        self.pending.insert(id, invitation);
    }
    fn command(&mut self, request: Request, jobs: &mut tokio::task::JoinSet<Job>) {
        let Request {
            operation,
            payload,
            reply,
        } = request;
        let immediate = match operation.as_str() {
            "snapshot" => Some(self.snapshot()),
            "add_device" => Some((|| {
                let input = text(&payload, "address")?.to_owned();
                meshdrop_core::discovery::parse_address(&input)?;
                if !self.manual.contains(&input) {
                    self.manual.push(input);
                }
                Ok(Value::Null)
            })()),
            "decide" => Some((|| {
                let id = text(&payload, "id")?;
                let invitation = self.pending.remove(id).context("invitation expired")?;
                let accepted = payload["accept"]
                    .as_bool()
                    .context("missing accept decision")?;
                invitation
                    .decision
                    .send(if accepted {
                        transfer::Decision::Accept
                    } else {
                        transfer::Decision::Reject
                    })
                    .map_err(|_| anyhow::anyhow!("invitation expired"))?;
                self.view["invitations"]
                    .as_array_mut()
                    .expect("array")
                    .retain(|item| item["id"] != id);
                Ok(Value::Null)
            })()),
            _ => None,
        };
        if let Some(result) = immediate {
            let _ = reply.send(result.map_err(|e| format!("{e:#}")));
            return;
        }
        let session = self.session.clone();
        let manual = self.manual.clone();
        let stop = self.stop.clone();
        jobs.spawn(async move {
            let work = session.execute(&operation, payload, manual);
            let result = tokio::select! { result = work => result, _ = stop.cancelled() => Err(anyhow::anyhow!("application closing")) };
            Job { operation, result, reply }
        });
    }
    fn snapshot(&self) -> Result<Value> {
        let mut view = self.view.clone();
        view["name"] = json!(self.session.config.name);
        view["fingerprint"] = json!(self.session.identity.fingerprint());
        view["port"] = json!(self.session.port);
        let address = advertised_address(self.session.port)?;
        let payload = meshdrop_core::discovery::qr(&address, &self.session.identity.fingerprint());
        let qr = qrcode::QrCode::new(payload.as_bytes())?;
        let matrix: Vec<Vec<bool>> = (0..qr.width())
            .map(|y| {
                (0..qr.width())
                    .map(|x| qr[(x, y)] == qrcode::Color::Dark)
                    .collect()
            })
            .collect();
        view["qr"] = json!(matrix);
        view["qr_content"] = json!(payload);
        Ok(view)
    }
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .with_context(|| format!("missing {key}"))
}
impl Session {
    async fn trust(&self) -> Result<meshdrop_core::identity::TrustStore> {
        let path = self.directory.join("trust.json");
        tokio::task::spawn_blocking(move || meshdrop_core::identity::TrustStore::load(&path))
            .await?
    }
    async fn execute(&self, operation: &str, payload: Value, manual: Vec<String>) -> Result<Value> {
        use meshdrop_core::{
            discovery::{self, DiscoveryProvider, Manual, PeerList, Tailscale},
            identity::{TrustStore, short_code},
            transfer::Client,
        };
        let trust = self.trust().await?;
        let timeout = Duration::from_secs(self.config.invitation_timeout_seconds + 5);
        match operation {
            "refresh" => {
                let mut providers: Vec<Box<dyn DiscoveryProvider>> = Vec::new();
                let mut addresses = manual;
                addresses.extend(trust.0.keys().cloned());
                // Explicit addresses remain usable even when optional providers are disabled.
                providers.push(Box::new(Manual {
                    addresses,
                    identity: self.identity.clone(),
                }));
                let mut setup_problems = Vec::new();
                for provider in &self.config.providers {
                    match provider.as_str() {
                        "mdns" => match Mdns::new(Duration::from_secs(2), self.identity.clone()) {
                            Ok(mdns) => providers.push(Box::new(mdns)),
                            Err(error) => setup_problems
                                .push(json!({"provider":"mdns", "error":format!("{error:#}")})),
                        },
                        "tailscale" => providers.push(Box::new(Tailscale {
                            identity: self.identity.clone(),
                            port: self.port,
                            command: "tailscale".into(),
                        })),
                        "peer-list" => providers.push(Box::new(PeerList {
                            identity: self.identity.clone(),
                            known: trust.0.clone(),
                        })),
                        "manual" => {}
                        unknown => setup_problems
                            .push(json!({"provider":unknown, "error":"unknown provider"})),
                    }
                }
                let report = discovery::discover(&providers).await;
                let devices = report
                    .devices
                    .into_iter()
                    .filter(|device| {
                        device.fingerprint.as_deref() != Some(&self.identity.fingerprint())
                    })
                    .map(|device| {
                        let target = device
                            .addresses
                            .iter()
                            .find(|address| {
                                device
                                    .fingerprint
                                    .as_ref()
                                    .is_some_and(|pin| trust.0.get(*address) == Some(pin))
                            })
                            .cloned();
                        let mismatch = device.addresses.iter().any(|address| {
                            trust
                                .0
                                .get(address)
                                .zip(device.fingerprint.as_ref())
                                .is_some_and(|(expected, actual)| expected != actual)
                        });
                        let mut value = serde_json::to_value(device).expect("device serialization");
                        value["paired"] = json!(target.is_some());
                        value["target"] = json!(target);
                        value["mismatch"] = json!(mismatch);
                        value
                    })
                    .collect::<Vec<_>>();
                let mut problems = serde_json::to_value(report.problems)?
                    .as_array()
                    .context("problems array")?
                    .clone();
                problems.extend(setup_problems);
                Ok(json!({"devices":devices,"problems":problems}))
            }
            "prepare_pair" => {
                let (address, qr_pin) = discovery::parse_address(text(&payload, "address")?)?;
                let known = trust.0.get(&address).cloned();
                if known
                    .as_ref()
                    .zip(qr_pin.as_ref())
                    .is_some_and(|(a, b)| a != b)
                {
                    bail!("fingerprint mismatch for {address}");
                }
                let info = Client::new(&address, &self.identity, known.or(qr_pin), timeout)?
                    .info()
                    .await?;
                Ok(
                    json!({"address":address,"fingerprint":info.fingerprint,"name":info.name,"code":short_code(&self.identity.fingerprint(),&info.fingerprint)}),
                )
            }
            "confirm_pair" => {
                let address = text(&payload, "address")?.to_owned();
                let pin = text(&payload, "fingerprint")?.to_owned();
                if trust
                    .0
                    .get(&address)
                    .is_some_and(|expected| expected != &pin)
                {
                    bail!("fingerprint mismatch for {address}");
                }
                let client = Client::new(&address, &self.identity, Some(pin.clone()), timeout)?
                    .with_name(self.config.name.clone());
                client
                    .pair(&local_address(&address, self.port).await?)
                    .await?;
                let path = self.directory.join("trust.json");
                tokio::task::spawn_blocking(move || {
                    TrustStore::load(&path)?.remember(&path, &address, &pin)
                })
                .await??;
                Ok(json!({"status":"paired"}))
            }
            _ => bail!("unknown desktop command {operation}"),
        }
    }
}
fn advertised_address(port: u16) -> Result<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    socket.connect("192.0.2.1:9")?;
    Ok(std::net::SocketAddr::new(socket.local_addr()?.ip(), port).to_string())
}
async fn local_address(peer: &str, port: u16) -> Result<String> {
    let destination = tokio::net::lookup_host(peer)
        .await?
        .next()
        .context("peer has no resolved address")?;
    let socket = tokio::net::UdpSocket::bind(if destination.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    })
    .await?;
    socket.connect(destination).await?;
    Ok(std::net::SocketAddr::new(socket.local_addr()?.ip(), port).to_string())
}
/// Resolves an explicit state directory for isolated profiles, or platform defaults.
pub fn state_directory() -> Result<PathBuf> {
    std::env::var_os("MESHDROP_STATE_DIR")
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(Config::directory)
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn profile(
        name: &str,
        providers: &str,
    ) -> (tempfile::TempDir, Controller, mpsc::Receiver<Notice>) {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("config.toml"),
            format!("port = 0\nproviders = {providers}\nname = '{name}'\n"),
        )
        .unwrap();
        let (tx, rx) = mpsc::channel(16);
        let controller = Controller::start(directory.path().to_owned(), tx)
            .await
            .unwrap();
        (directory, controller, rx)
    }
    async fn address(controller: &Controller) -> String {
        let snapshot = controller.call("snapshot", Value::Null).await.unwrap();
        format!("127.0.0.1:{}", snapshot["port"])
    }
    #[tokio::test]
    async fn discovery_retains_device_when_optional_provider_reports_problem() {
        let (_a, first, _notices_a) = profile("First", "['unknown']").await;
        let (_b, second, _notices_b) = profile("Second", "[]").await;
        first
            .call("add_device", json!({"address":address(&second).await}))
            .await
            .unwrap();
        let report = first.call("refresh", Value::Null).await.unwrap();
        assert_eq!(report["devices"][0]["name"], "Second");
        assert_eq!(report["devices"][0]["paired"], false);
        assert_eq!(report["problems"][0]["provider"], "unknown");
        first.shutdown().await;
        second.shutdown().await;
    }
    #[tokio::test]
    async fn pairing_waits_for_in_app_acceptance_and_persists_both_pins() {
        let (a, first, _notices_a) = profile("First", "[]").await;
        let (b, second, mut notices_b) = profile("Second", "[]").await;
        let target = address(&second).await;
        let prepared = first
            .call("prepare_pair", json!({"address":target}))
            .await
            .unwrap();
        assert_eq!(prepared["code"].as_str().unwrap().len(), 6);
        let sender = first.clone();
        let pairing = tokio::spawn(async move { sender.call("confirm_pair", prepared).await });
        notices_b.recv().await.unwrap();
        let snapshot = second.call("snapshot", Value::Null).await.unwrap();
        assert_eq!(snapshot["invitations"][0]["name"], "First");
        assert!(!pairing.is_finished());
        second
            .call(
                "decide",
                json!({"id":snapshot["invitations"][0]["id"],"accept":true}),
            )
            .await
            .unwrap();
        pairing.await.unwrap().unwrap();
        assert_eq!(
            meshdrop_core::identity::TrustStore::load(&a.path().join("trust.json"))
                .unwrap()
                .0
                .len(),
            1
        );
        assert_eq!(
            meshdrop_core::identity::TrustStore::load(&b.path().join("trust.json"))
                .unwrap()
                .0
                .len(),
            1
        );
        first.shutdown().await;
        second.shutdown().await;
    }
    #[tokio::test]
    async fn rejecting_pairing_does_not_persist_trust() {
        let (a, first, _notices_a) = profile("First", "[]").await;
        let (_b, second, mut notices_b) = profile("Second", "[]").await;
        let prepared = first
            .call("prepare_pair", json!({"address":address(&second).await}))
            .await
            .unwrap();
        let sender = first.clone();
        let pairing = tokio::spawn(async move { sender.call("confirm_pair", prepared).await });
        notices_b.recv().await.unwrap();
        let snapshot = second.call("snapshot", Value::Null).await.unwrap();
        second
            .call(
                "decide",
                json!({"id":snapshot["invitations"][0]["id"],"accept":false}),
            )
            .await
            .unwrap();
        assert!(pairing.await.unwrap().is_err());
        assert!(
            meshdrop_core::identity::TrustStore::load(&a.path().join("trust.json"))
                .unwrap()
                .0
                .is_empty()
        );
        first.shutdown().await;
        second.shutdown().await;
    }
    #[tokio::test]
    async fn qr_conflicting_with_stored_pin_reports_fingerprint_mismatch() {
        let (a, first, _notices_a) = profile("First", "[]").await;
        meshdrop_core::identity::TrustStore::default()
            .remember(&a.path().join("trust.json"), "localhost:1", &"a".repeat(64))
            .unwrap();
        let qr = meshdrop_core::discovery::qr("localhost:1", &"b".repeat(64));
        let error = first
            .call("prepare_pair", json!({"address":qr}))
            .await
            .unwrap_err();
        assert!(error.contains("fingerprint mismatch"));
        first.shutdown().await;
    }
    #[tokio::test]
    async fn background_server_uses_profile_identity_and_stops_cleanly() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("config.toml"),
            "port = 0\nproviders = []\nname = 'Desktop test'\n",
        )
        .unwrap();
        let (tx, _rx) = mpsc::channel(4);
        let controller = Controller::start(directory.path().to_owned(), tx)
            .await
            .unwrap();
        let snapshot = controller.call("snapshot", Value::Null).await.unwrap();
        let client_identity = Identity::load(&directory.path().join("client")).unwrap();
        let address = format!("127.0.0.1:{}", snapshot["port"]);
        let info = transfer::Client::new(
            &address,
            &client_identity,
            Some(snapshot["fingerprint"].as_str().unwrap().into()),
            Duration::from_secs(2),
        )
        .unwrap()
        .info()
        .await
        .unwrap();
        assert_eq!(info.name, "Desktop test");
        controller.shutdown().await;
        assert!(tokio::net::TcpListener::bind(&address).await.is_ok());
    }
    #[tokio::test]
    async fn invalid_configuration_returns_path_before_starting_server() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("config.toml"), "prot = 3").unwrap();
        let (tx, _rx) = mpsc::channel(4);
        let error = Controller::start(directory.path().to_owned(), tx)
            .await
            .err()
            .unwrap();
        assert!(format!("{error:#}").contains("config.toml"));
    }
}
