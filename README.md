# meshdrop

Private-network file invitations for Linux, Windows and macOS. Rust core and CLI; Tauri and Android are later stages. GPL-3.0-or-later.

```sh
cargo build --workspace
cargo run --bin meshdrop -- --help
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Transfers require explicit acceptance and pinned device certificates. See [protocol](docs/protocol.md) and [progress](progress.md). No cloud service is needed.
