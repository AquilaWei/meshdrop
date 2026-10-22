use async_trait::async_trait;
use meshdrop_core::discovery::{Device, DiscoveryProvider, discover, parse_address, qr};
struct Offline;
struct Online;
#[async_trait]
impl DiscoveryProvider for Offline {
    async fn discover(&self) -> anyhow::Result<Vec<Device>> {
        Ok(vec![Device {
            name: "old".into(),
            platform: "unknown".into(),
            address: "127.0.0.1:9843".into(),
            online: false,
        }])
    }
}
#[async_trait]
impl DiscoveryProvider for Online {
    async fn discover(&self) -> anyhow::Result<Vec<Device>> {
        Ok(vec![Device {
            name: "new".into(),
            platform: "linux".into(),
            address: "127.0.0.1:9843".into(),
            online: true,
        }])
    }
}
#[tokio::test]
async fn providers_merge_and_prefer_online_metadata() {
    let devices = discover(&[Box::new(Offline), Box::new(Online)])
        .await
        .unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].name, "new");
    assert!(devices[0].online);
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
