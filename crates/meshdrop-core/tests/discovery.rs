use async_trait::async_trait;
use meshdrop_core::discovery::{
    Device, DiscoveryProvider, DiscoveryReport, discover, parse_address, qr,
};
struct Offline;
struct Online;
#[async_trait]
impl DiscoveryProvider for Offline {
    async fn discover(&self) -> anyhow::Result<DiscoveryReport> {
        Ok(vec![Device {
            name: "old".into(),
            platform: "unknown".into(),
            address: "127.0.0.1:9843".into(),
            online: false,
        }]
        .into())
    }
}
#[async_trait]
impl DiscoveryProvider for Online {
    async fn discover(&self) -> anyhow::Result<DiscoveryReport> {
        Ok(vec![Device {
            name: "new".into(),
            platform: "linux".into(),
            address: "127.0.0.1:9843".into(),
            online: true,
        }]
        .into())
    }
}
#[tokio::test]
async fn providers_merge_and_prefer_online_metadata() {
    let devices = discover(&[Box::new(Offline), Box::new(Online)]).await;
    assert_eq!(devices.devices.len(), 1);
    assert_eq!(devices.devices[0].name, "new");
    assert!(devices.devices[0].online);
}
#[test]
fn qr_roundtrip_retains_pin() {
    assert_eq!(
        parse_address(&qr("host:9843", &"ab".repeat(32))).unwrap(),
        ("host:9843".into(), Some("ab".repeat(32)))
    );
}
#[test]
fn invalid_qr_version_is_rejected() {
    assert!(parse_address("meshdrop://host:9843#v=2&fp=aa").is_err());
}
struct Broken;
#[async_trait]
impl DiscoveryProvider for Broken {
    async fn discover(&self) -> anyhow::Result<DiscoveryReport> {
        anyhow::bail!("provider unavailable")
    }
}
#[tokio::test]
async fn failed_provider_preserves_other_devices() {
    let devices = discover(&[Box::new(Broken), Box::new(Online)]).await;
    assert_eq!(devices.devices.len(), 1);
    assert_eq!(devices.devices[0].name, "new");
    assert_eq!(devices.problems.len(), 1);
    assert!(devices.problems[0].error.contains("provider unavailable"));
}

const TAILSCALE_STATUS: &[u8] = br#"{
 "Peer": {
  "first": {"HostName":"v4-peer","OS":"linux","Online":true,"TailscaleIPs":["100.64.1.2"]},
  "second": {"HostName":"v6-peer","OS":"windows","Online":true,"TailscaleIPs":["fd7a:115c:a1e0::2"]},
  "third": {"HostName":"offline-peer","OS":"macOS","Online":false,"TailscaleIPs":["100.64.1.3"]}
 }
}"#;
#[test]
fn tailscale_status_preserves_ipv4_address() {
    let devices = meshdrop_core::discovery::parse_tailscale_status(TAILSCALE_STATUS, 9843).unwrap();
    assert_eq!(devices[0].address, "100.64.1.2:9843");
}
#[test]
fn tailscale_status_brackets_ipv6_address() {
    let devices = meshdrop_core::discovery::parse_tailscale_status(TAILSCALE_STATUS, 9843).unwrap();
    assert_eq!(devices[1].address, "[fd7a:115c:a1e0::2]:9843");
}
#[test]
fn tailscale_status_keeps_offline_peers_offline() {
    let devices = meshdrop_core::discovery::parse_tailscale_status(TAILSCALE_STATUS, 9843).unwrap();
    assert!(!devices[2].online);
}
#[test]
fn tailscale_status_without_peers_is_empty() {
    assert!(
        meshdrop_core::discovery::parse_tailscale_status(br#"{"BackendState":"Running"}"#, 9843)
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn missing_tailscale_reports_injected_executable_path() {
    let directory = tempfile::tempdir().unwrap();
    let command = directory.path().join("tailscale-does-not-exist");
    let provider = meshdrop_core::discovery::Tailscale {
        identity: std::sync::Arc::new(
            meshdrop_core::identity::Identity::load(directory.path()).unwrap(),
        ),
        port: 9843,
        command: command.clone(),
    };
    assert!(
        provider
            .discover()
            .await
            .unwrap_err()
            .to_string()
            .contains(&command.display().to_string())
    );
}
