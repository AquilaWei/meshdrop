//! Pluggable discovery: advertisements never grant trust.
use crate::{identity::Identity, transfer::Client};
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
/// DNS-SD uses the required service type with its local domain suffix.
pub const SERVICE: &str = "_meshdrop._tcp.local.";
/// A candidate may be offline; discovery data is not authentication.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Device {
    pub name: String,
    pub platform: String,
    pub address: String,
    pub online: bool,
}
/// Partial results keep optional integrations from hiding reachable devices.
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct DiscoveryReport {
    pub devices: Vec<Device>,
    pub problems: Vec<DiscoveryProblem>,
}
/// Diagnostics name the integration and preserve the original failure reason.
#[derive(Debug, Serialize, Deserialize)]
pub struct DiscoveryProblem {
    pub provider: String,
    pub error: String,
}
impl From<Vec<Device>> for DiscoveryReport {
    fn from(devices: Vec<Device>) -> Self {
        Self {
            devices,
            problems: vec![],
        }
    }
}
/// Providers can return partial results; complete setup failures remain errors.
#[async_trait]
pub trait DiscoveryProvider: Send + Sync {
    /// Stable diagnostic name; frontends can show failures independently of results.
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    /// Returns candidates without changing trust. Partial failures belong in problems.
    async fn discover(&self) -> Result<DiscoveryReport>;
}
/// Merges all usable results while retaining diagnostics from every provider.
pub async fn discover(providers: &[Box<dyn DiscoveryProvider>]) -> DiscoveryReport {
    let mut merged = BTreeMap::new();
    let mut problems = Vec::new();
    for provider in providers {
        match provider.discover().await {
            Ok(report) => {
                problems.extend(report.problems);
                for device in report.devices {
                    let old = merged
                        .entry(device.address.clone())
                        .or_insert_with(|| device.clone());
                    if device.online {
                        *old = device;
                    }
                }
            }
            Err(error) => problems.push(DiscoveryProblem {
                provider: provider.name().into(),
                error: format!("{error:#}"),
            }),
        }
    }
    DiscoveryReport {
        devices: merged.into_values().collect(),
        problems,
    }
}
/// Manual inputs and QR payloads retain the pin for explicit pairing, separately
/// from the candidate's untrusted descriptive metadata.
pub struct Manual {
    pub addresses: Vec<String>,
    pub identity: Arc<Identity>,
}
/// QR payloads use a single versioned format shared by the CLI and future apps.
pub fn qr(address: &str, fingerprint: &str) -> String {
    format!("meshdrop://{address}#v=1&fp={fingerprint}")
}
/// Parses manual host:port or QR content. Invalid QR versions/pins fail explicitly.
pub fn parse_address(input: &str) -> Result<(String, Option<String>)> {
    if let Some(rest) = input.strip_prefix("meshdrop://") {
        let (address, pin) = rest
            .split_once("#v=1&fp=")
            .context("invalid QR payload or unsupported version")?;
        if pin.len() != 64 || hex::decode(pin).is_err() {
            bail!("invalid QR fingerprint");
        }
        return Ok((address.into(), Some(pin.into())));
    }
    if input.contains('/') || input.is_empty() {
        bail!("expected host:port or meshdrop QR payload");
    }
    Ok((input.into(), None))
}
async fn probe(address: String, identity: &Identity) -> Device {
    let result = async {
        Client::new(&address, identity, None, Duration::from_secs(2))?
            .info()
            .await
    }
    .await;
    match result {
        Ok(info) => Device {
            name: info.name,
            platform: info.platform,
            address,
            online: true,
        },
        Err(_) => Device {
            name: address.clone(),
            platform: "unknown".into(),
            address,
            online: false,
        },
    }
}
#[async_trait]
impl DiscoveryProvider for Manual {
    fn name(&self) -> &'static str {
        "manual"
    }
    async fn discover(&self) -> Result<DiscoveryReport> {
        let mut devices = Vec::new();
        for input in &self.addresses {
            let (address, _) = parse_address(input)?;
            devices.push(probe(address, &self.identity).await);
        }
        Ok(devices.into())
    }
}
/// Tailscale is optional and accessed via its documented local CLI JSON output.
pub struct Tailscale {
    pub identity: Arc<Identity>,
    pub port: u16,
}
#[async_trait]
impl DiscoveryProvider for Tailscale {
    fn name(&self) -> &'static str {
        "tailscale"
    }
    async fn discover(&self) -> Result<DiscoveryReport> {
        let output = tokio::process::Command::new("tailscale")
            .args(["status", "--json"])
            .output()
            .await
            .context("cannot run tailscale: install it or disable the tailscale provider")?;
        if !output.status.success() {
            bail!(
                "tailscale status failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let mut devices = Vec::new();
        if let Some(peers) = json["Peer"].as_object() {
            for peer in peers.values() {
                if let Some(ip) = peer["TailscaleIPs"]
                    .as_array()
                    .and_then(|ips| ips.first())
                    .and_then(|ip| ip.as_str())
                {
                    let address = if ip.contains(':') {
                        format!("[{ip}]:{}", self.port)
                    } else {
                        format!("{ip}:{}", self.port)
                    };
                    devices.push(probe(address, &self.identity).await);
                }
            }
        }
        Ok(devices.into())
    }
}
/// Peer lists extend discovery through already authenticated peers only.
pub struct PeerList {
    pub known: BTreeMap<String, String>,
    pub identity: Arc<Identity>,
}
#[async_trait]
impl DiscoveryProvider for PeerList {
    fn name(&self) -> &'static str {
        "peer-list"
    }
    async fn discover(&self) -> Result<DiscoveryReport> {
        let mut report = DiscoveryReport::default();
        for (address, pin) in &self.known {
            let response = async {
                Client::new(
                    address,
                    &self.identity,
                    Some(pin.clone()),
                    Duration::from_secs(3),
                )?
                .peers()
                .await
            }
            .await;
            match response {
                Ok(peers) => {
                    for candidate in peers.into_keys() {
                        report.devices.push(probe(candidate, &self.identity).await);
                    }
                }
                Err(error) => report.problems.push(DiscoveryProblem {
                    provider: self.name().into(),
                    error: format!("{address}: {error:#}"),
                }),
            }
        }
        Ok(report)
    }
}
/// mDNS advertisements are held alive by this owner and explicitly shut down.
pub struct Mdns {
    daemon: ServiceDaemon,
    duration: Duration,
}
impl Mdns {
    /// Creates a LAN browser. Socket setup errors propagate to the frontend.
    pub fn new(duration: Duration) -> Result<Self> {
        Ok(Self {
            daemon: ServiceDaemon::new()?,
            duration,
        })
    }
    /// Advertises on all local interfaces; the fingerprint remains a TLS concern.
    pub fn advertise(&self, name: &str, port: u16) -> Result<()> {
        let properties = [("name", name), ("platform", std::env::consts::OS)];
        let service = ServiceInfo::new(
            SERVICE,
            name,
            &format!("meshdrop-{}.local.", uuid::Uuid::new_v4()),
            "",
            port,
            &properties[..],
        )?
        .enable_addr_auto();
        self.daemon.register(service)?;
        Ok(())
    }
    /// Requests daemon termination. Callers report errors instead of ignoring shutdown.
    pub fn shutdown(&self) -> Result<()> {
        self.daemon.shutdown()?;
        Ok(())
    }
}
#[async_trait]
impl DiscoveryProvider for Mdns {
    fn name(&self) -> &'static str {
        "mdns"
    }
    async fn discover(&self) -> Result<DiscoveryReport> {
        let receiver = self.daemon.browse(SERVICE)?;
        let until = tokio::time::Instant::now() + self.duration;
        let mut devices = Vec::new();
        while let Ok(Ok(event)) = tokio::time::timeout_at(until, receiver.recv_async()).await {
            if let ServiceEvent::ServiceResolved(info) = event {
                for ip in info.get_addresses() {
                    let address = std::net::SocketAddr::new(*ip, info.get_port()).to_string();
                    devices.push(Device {
                        name: info
                            .get_property_val_str("name")
                            .unwrap_or(info.get_fullname())
                            .into(),
                        platform: info
                            .get_property_val_str("platform")
                            .unwrap_or("unknown")
                            .into(),
                        address,
                        online: true,
                    });
                }
            }
        }
        self.daemon.stop_browse(SERVICE)?;
        Ok(devices.into())
    }
}
