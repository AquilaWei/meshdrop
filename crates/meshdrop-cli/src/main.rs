//! Command-line frontend; all policy and transport live in meshdrop-core.
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use meshdrop_core::{
    config::Config,
    discovery::{self, DiscoveryProvider, Manual, Mdns, PeerList, Tailscale},
    identity::{Identity, TrustStore, short_code},
    transfer::{self, Client, Decision, InvitationKind},
};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
#[derive(Parser)]
#[command(version, about = "Private-network file invitations over pinned HTTPS")]
struct Args {
    #[arg(long, global = true)]
    state_dir: Option<PathBuf>,
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Id {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },
    Pair {
        addr: String,
        #[arg(long)]
        advertise: Option<String>,
    },
    Devices {
        addresses: Vec<String>,
    },
    Receive {
        #[arg(long, conflicts_with = "auto_reject")]
        auto_accept: bool,
        #[arg(long)]
        auto_reject: bool,
    },
    Send {
        device: String,
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
}
async fn confirm(prompt: String) -> Result<bool> {
    tokio::task::spawn_blocking(move || {
        eprintln!("{prompt} [y/N]");
        let mut line = String::new();
        std::io::stdin().read_line(&mut line)?;
        Ok(line.trim().eq_ignore_ascii_case("y"))
    })
    .await?
}
async fn signal(token: CancellationToken) -> Result<()> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! { result = tokio::signal::ctrl_c() => result?, _ = terminate.recv() => {} }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await?;
    token.cancel();
    Ok(())
}
fn main() -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(async_main());
    // A terminal read may still be blocked when a termination signal arrives.
    runtime.shutdown_timeout(Duration::from_millis(100));
    result
}
async fn async_main() -> Result<()> {
    let args = Args::parse();
    let directory = args.state_dir.unwrap_or(Config::directory()?);
    std::fs::create_dir_all(&directory)?;
    let config = Config::load(&args.config.unwrap_or(directory.join("config.toml")))?;
    let identity = Arc::new(Identity::load(&directory)?);
    let shutdown = CancellationToken::new();
    let mut signals = tokio::spawn(signal(shutdown.clone()));
    let work = run(args.command, config, directory, identity, shutdown.clone());
    tokio::pin!(work);
    tokio::select! {
        result = &mut work => { signals.abort(); result }
        result = &mut signals => {
            result??;
            // Give transport cleanup time, even when an interactive terminal read blocks.
            match tokio::time::timeout(Duration::from_secs(2), &mut work).await {
                Ok(result) => result,
                Err(_) => Ok(()),
            }
        }
    }
}
async fn run(
    command: Command,
    config: Config,
    directory: PathBuf,
    identity: Arc<Identity>,
    stop: CancellationToken,
) -> Result<()> {
    let timeout = Duration::from_secs(config.invitation_timeout_seconds + 5);
    let trust_path = directory.join("trust.json");
    let mut trust = TrustStore::load(&trust_path)?;
    match command {
        Command::Id { host } => {
            let address = format!("{host}:{}", config.port);
            let payload = discovery::qr(&address, &identity.fingerprint());
            println!("{}\n{payload}", identity.fingerprint());
            println!(
                "{}",
                qrcode::QrCode::new(payload.as_bytes())?
                    .render::<qrcode::render::unicode::Dense1x2>()
                    .quiet_zone(true)
                    .build()
            );
        }
        Command::Pair { addr, advertise } => {
            let (address, qr_pin) = discovery::parse_address(&addr)?;
            let known_pin = trust.0.get(&address).cloned();
            if qr_pin
                .as_ref()
                .zip(known_pin.as_ref())
                .is_some_and(|(a, b)| a != b)
            {
                bail!("fingerprint mismatch for {address}");
            }
            let pin = known_pin.or(qr_pin);
            let info = Client::new(&address, &identity, pin, timeout)?
                .info()
                .await?;
            let code = short_code(&identity.fingerprint(), &info.fingerprint);
            if !confirm(format!(
                "Pair with {} ({address})? Compare code {code} on BOTH devices",
                info.name
            ))
            .await?
            {
                bail!("pairing cancelled");
            }
            let client = Client::new(&address, &identity, Some(info.fingerprint.clone()), timeout)?
                .with_name(config.name.clone());
            client
                .pair(&advertise.unwrap_or(local_address(&address, config.port).await?))
                .await?;
            trust.remember(&trust_path, &address, &info.fingerprint)?;
            println!("paired {address}");
        }
        Command::Devices { addresses } => {
            let mut providers: Vec<Box<dyn DiscoveryProvider>> = vec![];
            for provider in &config.providers {
                match provider.as_str() {
                    "mdns" => match Mdns::new(Duration::from_secs(3), identity.clone()) {
                        Ok(provider) => providers.push(Box::new(provider)),
                        Err(error) => eprintln!("discovery mdns: {error:#}"),
                    },
                    "manual" => providers.push(Box::new(Manual {
                        addresses: addresses.clone(),
                        identity: identity.clone(),
                    })),
                    "tailscale" => providers.push(Box::new(Tailscale {
                        identity: identity.clone(),
                        port: config.port,
                        command: "tailscale".into(),
                    })),
                    "peer-list" => providers.push(Box::new(PeerList {
                        identity: identity.clone(),
                        known: trust.0.clone(),
                    })),
                    _ => bail!("unknown discovery provider {provider}"),
                }
            }
            let report = discovery::discover(&providers).await;
            for problem in report.problems {
                eprintln!("discovery {}: {}", problem.provider, problem.error);
            }
            println!("{}", serde_json::to_string_pretty(&report.devices)?);
        }
        Command::Receive {
            auto_accept,
            auto_reject,
        } => {
            let mdns = if config.providers.iter().any(|p| p == "mdns") {
                let mdns = Mdns::new(Duration::from_secs(3), identity.clone())?;
                mdns.advertise(&config.name, config.port)?;
                Some(mdns)
            } else {
                None
            };
            let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.port)).await?;
            eprintln!("listening {}", listener.local_addr()?);
            let (tx, mut invitations) = mpsc::channel::<transfer::Invitation>(16);
            let (progress, mut events) = mpsc::channel::<transfer::Progress>(16);
            let frontend_stop = stop.clone();
            let frontend = tokio::spawn(async move {
                loop {
                    tokio::select! {
                     _ = frontend_stop.cancelled() => break,
                     Some(event) = events.recv() => eprintln!("receive {} {}/{}",event.name,event.bytes,event.total),
                     Some(invitation) = invitations.recv() => {
                      let prompt = match &invitation.kind { InvitationKind::Pair { code } => format!("Pair {}? Compare code {code} on BOTH devices",invitation.peer), InvitationKind::Files(files) => format!("Accept from {}: {files:?}?",invitation.peer) };
                      // Pairing always requires confirmation; automation feeds explicit stdin confirmations.
                      let accepted = match invitation.kind { InvitationKind::Pair { .. } => tokio::select! { result = confirm(prompt) => result?, _ = frontend_stop.cancelled() => break }, _ if auto_accept => true, _ if auto_reject => false, _ => tokio::select! { result = confirm(prompt) => result?, _ = frontend_stop.cancelled() => break } };
                      if invitation.decision.send(if accepted { Decision::Accept } else { Decision::Reject }).is_err() { eprintln!("invitation expired while awaiting confirmation"); }
                     }
                    }
                }
                Ok::<_, anyhow::Error>(())
            });
            let result = transfer::serve(
                listener,
                config,
                directory,
                identity,
                tx,
                progress,
                stop.clone(),
            )
            .await;
            stop.cancel();
            frontend.await??;
            if let Some(mdns) = mdns {
                mdns.shutdown()?;
            }
            result?;
        }
        Command::Send { device, files } => {
            let (address, _) = discovery::parse_address(&device)?;
            let pin = trust
                .0
                .get(&address)
                .context("unpaired device; run meshdrop pair first")?
                .clone();
            let client = Client::new(&address, &identity, Some(pin), timeout)?
                .with_name(config.name.clone());
            let (tx, mut rx) = mpsc::channel::<transfer::Progress>(16);
            let display = tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    eprintln!("send {} {}/{}", event.name, event.bytes, event.total);
                }
            });
            let result = client.send(&files, tx, stop).await;
            display.await?;
            result?;
            println!("transfer complete");
        }
    }
    Ok(())
}

async fn local_address(peer: &str, port: u16) -> Result<String> {
    let destination = tokio::net::lookup_host(peer)
        .await?
        .next()
        .context("peer has no resolved address")?;
    let bind = if destination.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = tokio::net::UdpSocket::bind(bind).await?;
    socket.connect(destination).await?;
    Ok(std::net::SocketAddr::new(socket.local_addr()?.ip(), port).to_string())
}
