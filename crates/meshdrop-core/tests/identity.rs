use meshdrop_core::identity::{Identity, TrustStore, short_code};
#[test]
fn identity_survives_restart() {
    let dir = tempfile::tempdir().unwrap();
    let first = Identity::load(dir.path()).unwrap();
    assert_eq!(
        first.fingerprint(),
        Identity::load(dir.path()).unwrap().fingerprint()
    );
}
#[test]
fn pairing_survives_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("trust.json");
    TrustStore::default()
        .remember(&path, "peer", "pin")
        .unwrap();
    TrustStore::load(&path)
        .unwrap()
        .verify("peer", "pin")
        .unwrap();
}
#[test]
fn changed_fingerprint_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut trust = TrustStore::default();
    trust
        .remember(&dir.path().join("trust.json"), "peer", "old")
        .unwrap();
    assert!(
        trust
            .verify("peer", "new")
            .unwrap_err()
            .to_string()
            .contains("fingerprint mismatch")
    );
}
#[test]
fn short_code_is_symmetric() {
    assert_eq!(short_code("alice", "bob"), short_code("bob", "alice"));
}
