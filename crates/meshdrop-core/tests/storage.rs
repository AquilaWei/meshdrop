use meshdrop_core::{
    config::Config,
    sink::{Sink, sanitize},
};
use sha2::{Digest, Sha256};
#[test]
fn strips_both_path_separators() {
    assert_eq!(sanitize("../foo\\bar.txt").unwrap(), "bar.txt");
}
#[test]
fn replaces_windows_characters() {
    assert_eq!(
        sanitize("a<>:\"|?*\u{0001}.txt. ").unwrap(),
        "a________.txt"
    );
}
#[test]
fn protects_reserved_first_component() {
    assert_eq!(sanitize("COM¹.more.txt").unwrap(), "_COM¹.more.txt");
}
#[test]
fn rejects_relative_names() {
    assert!(sanitize("..").is_err());
}
#[test]
fn truncates_utf8_and_preserves_extension() {
    assert_eq!(
        sanitize(&format!("{}.txt", "é".repeat(100))).unwrap(),
        format!("{}.txt", "é".repeat(98))
    );
}
#[test]
fn never_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "old").unwrap();
    let mut sink = Sink::new(dir.path(), "a.txt").unwrap();
    sink.write(b"new").unwrap();
    assert_eq!(
        sink.finish(3, &hex::encode(Sha256::digest(b"new")))
            .unwrap(),
        dir.path().join("a (1).txt")
    );
    assert_eq!(std::fs::read(dir.path().join("a.txt")).unwrap(), b"old");
}
#[test]
fn failed_digest_cleans_temporary_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut sink = Sink::new(dir.path(), "a").unwrap();
    sink.write(b"new").unwrap();
    assert!(sink.finish(3, "incorrect").is_err());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
#[test]
fn dropping_sink_cleans_temporary_file() {
    let dir = tempfile::tempdir().unwrap();
    let sink = Sink::new(dir.path(), "a").unwrap();
    drop(sink);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
#[test]
fn public_source_is_denied() {
    assert!(!Config::default().allows("8.8.8.8".parse().unwrap()));
}
#[test]
fn mapped_private_source_is_allowed() {
    assert!(Config::default().allows("::ffff:192.168.1.2".parse().unwrap()));
}
#[test]
fn unknown_configuration_key_reports_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "prot = 3").unwrap();
    assert!(
        Config::load(&path)
            .unwrap_err()
            .to_string()
            .contains(&path.display().to_string())
    );
}

fn occupy_collision_candidates(directory: &std::path::Path) {
    std::fs::write(directory.join("full.txt"), b"existing").unwrap();
    for number in 1..10_000 {
        std::fs::write(directory.join(format!("full ({number}).txt")), b"existing").unwrap();
    }
}
#[test]
fn exhausted_collision_candidates_fail_and_remove_temporary_file() {
    let directory = tempfile::tempdir().unwrap();
    occupy_collision_candidates(directory.path());
    let mut sink = Sink::new(directory.path(), "full.txt").unwrap();
    sink.write(b"new").unwrap();
    let result = sink.finish(3, &hex::encode(Sha256::digest(b"new")));
    assert!(result.unwrap_err().to_string().contains("collision limit"));
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 10_000);
    assert_eq!(
        std::fs::read(directory.path().join("full.txt")).unwrap(),
        b"existing"
    );
}
