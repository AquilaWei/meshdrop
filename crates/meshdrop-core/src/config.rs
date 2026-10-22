//! Strict, portable configuration and source-network policy.
use anyhow::{Context, Result};
use directories::ProjectDirs;
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use std::{
    net::IpAddr,
    path::{Path, PathBuf},
};
/// Configuration is strict to make misspelled security settings fail closed.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub receive_directory: PathBuf,
    pub port: u16,
    pub allowed_networks: Vec<IpNet>,
    pub providers: Vec<String>,
    pub invitation_timeout_seconds: u64,
    /// Maximum gap between received chunks or between authorized uploads.
    pub idle_timeout_seconds: u64,
    pub name: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            receive_directory: PathBuf::from("received"),
            port: 9843,
            allowed_networks: [
                "10.0.0.0/8",
                "172.16.0.0/12",
                "192.168.0.0/16",
                "100.64.0.0/10",
                "169.254.0.0/16",
                "127.0.0.0/8",
                "::1/128",
                "fc00::/7",
                "fe80::/10",
            ]
            .map(|n| n.parse().expect("constant network"))
            .into(),
            providers: vec!["mdns".into(), "manual".into(), "peer-list".into()],
            invitation_timeout_seconds: 60,
            idle_timeout_seconds: 60,
            name: "meshdrop".into(),
        }
    }
}
impl Config {
    /// Reads TOML with a filename on every error; missing files use safe defaults.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text)
                .with_context(|| format!("invalid configuration {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("read configuration {}", path.display())),
        }
    }
    /// Resolves the platform's configuration directory rather than assuming Unix.
    pub fn directory() -> Result<PathBuf> {
        Ok(ProjectDirs::from("org", "meshdrop", "meshdrop")
            .context("platform has no configuration directory")?
            .config_dir()
            .to_owned())
    }
    /// Normalizes mapped IPv6 addresses so IPv4 policy cannot be bypassed.
    pub fn allows(&self, address: IpAddr) -> bool {
        let normalized = match address {
            IpAddr::V6(v) => v.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(address),
            _ => address,
        };
        self.allowed_networks
            .iter()
            .any(|network| network.contains(&normalized))
    }
}
