//! Strict, portable configuration and source-network policy.
use anyhow::{Context, Result};
use directories::ProjectDirs;
#[cfg(not(target_os = "android"))]
use directories::UserDirs;
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
    /// Desktop notifications can be disabled without affecting invitation decisions.
    pub notifications_enabled: bool,
}
/// Uses the OS Downloads setting (including Linux XDG); falls back to home/Downloads.
/// A home directory is required by all supported platforms.
pub fn default_receive_directory() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        PathBuf::from("received")
    }
    #[cfg(not(target_os = "android"))]
    {
        let user = UserDirs::new().expect("platform has no home directory");
        downloads_or_home(user.download_dir(), user.home_dir())
    }
}
#[cfg(not(target_os = "android"))]
fn downloads_or_home(download: Option<&Path>, home: &Path) -> PathBuf {
    download
        .map(Path::to_path_buf)
        .unwrap_or_else(|| home.join("Downloads"))
}
impl Default for Config {
    fn default() -> Self {
        Self {
            receive_directory: default_receive_directory(),
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
            notifications_enabled: true,
        }
    }
}
impl Config {
    /// Reads TOML with a filename on every error; missing files use safe defaults.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let mut config: Self = toml::from_str(&text)
                    .with_context(|| format!("invalid configuration {}", path.display()))?;
                if config.receive_directory.is_relative() {
                    // Relative user paths belong to the config, never the launch directory.
                    let parent = std::path::absolute(path)?
                        .parent()
                        .context("configuration has no parent")?
                        .to_owned();
                    config.receive_directory = parent.join(&config.receive_directory);
                }
                Ok(config)
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "android"))]
    #[test]
    fn missing_downloads_setting_falls_back_to_home_downloads() {
        assert_eq!(
            downloads_or_home(None, Path::new("home")),
            PathBuf::from("home/Downloads")
        );
    }

    #[cfg(not(target_os = "android"))]
    #[test]
    fn platform_downloads_setting_takes_priority_over_home() {
        assert_eq!(
            downloads_or_home(Some(Path::new("xdg-downloads")), Path::new("home")),
            PathBuf::from("xdg-downloads")
        );
    }

    #[test]
    fn configured_relative_directory_is_relative_to_config_parent() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("config.toml");
        std::fs::write(&path, "receive_directory = 'incoming'\n").unwrap();
        assert_eq!(
            Config::load(&path).unwrap().receive_directory,
            folder.path().join("incoming")
        );
    }

    #[test]
    fn explicit_absolute_directory_keeps_priority() {
        let folder = tempfile::tempdir().unwrap();
        let destination = folder.path().join("chosen");
        let path = folder.path().join("config.toml");
        let config = Config {
            receive_directory: destination.clone(),
            ..Config::default()
        };
        std::fs::write(&path, toml::to_string(&config).unwrap()).unwrap();
        assert_eq!(Config::load(&path).unwrap().receive_directory, destination);
    }

    #[cfg(not(target_os = "android"))]
    #[test]
    fn default_receive_directory_is_platform_downloads_not_process_cwd() {
        let user = directories::UserDirs::new().unwrap();
        let expected = user
            .download_dir()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| user.home_dir().join("Downloads"));
        assert_eq!(Config::default().receive_directory, expected);
        assert!(Config::default().receive_directory.is_absolute());
    }
}
