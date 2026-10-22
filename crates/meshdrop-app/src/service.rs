//! One actor owns frontend state; bounded channels serialize decisions and updates.
use anyhow::{Result, bail};
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
            directory,
            config,
            identity,
            port,
            stop: stop.clone(),
            notices,
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
struct Actor {
    directory: PathBuf,
    config: Config,
    identity: Arc<Identity>,
    port: u16,
    stop: CancellationToken,
    notices: mpsc::Sender<Notice>,
}
impl Actor {
    async fn run(
        self,
        mut requests: mpsc::Receiver<Request>,
        mut invitations: mpsc::Receiver<Invitation>,
        mut progress: mpsc::Receiver<Progress>,
    ) {
        loop {
            tokio::select! {
                _ = self.stop.cancelled() => break,
                Some(request) = requests.recv() => {
                    let result = self.command(&request.operation, request.payload).await.map_err(|e| format!("{e:#}"));
                    let _ = request.reply.send(result);
                },
                Some(invitation) = invitations.recv() => { drop(invitation); },
                Some(_event) = progress.recv() => {},
            }
        }
        self.stop.cancel();
    }
    async fn command(&self, operation: &str, _payload: Value) -> Result<Value> {
        match operation {
            "snapshot" => Ok(
                json!({"name": self.config.name, "fingerprint": self.identity.fingerprint(), "port": self.port, "directory": self.directory, "notifications_ready": !self.notices.is_closed()}),
            ),
            _ => bail!("unknown desktop command {operation}"),
        }
    }
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
