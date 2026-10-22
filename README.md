# meshdrop

**Private-network file invitations for Linux, Windows and macOS.** No cloud,
public CA or Tailscale account required. The Rust core is shared by the CLI and
future Tauri/Android frontends. GPL-3.0-or-later; see [LICENSE](LICENSE).

## Try it

Install stable Rust with Cargo, then:

```sh
cargo build --workspace --locked
cargo run --bin meshdrop -- --help
# On the receiving machine (leave this running):
cargo run --bin meshdrop -- receive
# In another terminal, show your identity and terminal QR:
cargo run --bin meshdrop -- id --host 192.168.1.10
# On the sender; compare the six-digit code on BOTH machines and answer y:
cargo run --bin meshdrop -- pair 192.168.1.10:9843
cargo run --bin meshdrop -- devices
cargo run --bin meshdrop -- send 192.168.1.10:9843 ./README.md ./LICENSE
```

Replace the example address with the receiver's reachable IP. `pair` also accepts
the full `meshdrop://…` QR payload. Both users must confirm pairing. A changed
certificate is rejected with a fingerprint mismatch error. Keep the identity
file private and do not copy it between devices.

`receive --auto-accept` accepts file invitations, while `--auto-reject` rejects
them. Both still require manual pairing confirmation. Without either flag, the
receiver prompts for each offer. Ctrl-C or SIGTERM terminates reception and cleans
partial files. Completed files are SHA-256 verified and never overwrite existing
names. Transfers stream in bounded chunks and report progress on stderr.

For two local instances, use `--state-dir /path/to/private/state` on every command
and set different ports in their config files. `pair --advertise host:port` sets
a return address explicitly; otherwise the CLI derives it from the route to the
peer. Firewall rules must permit the configured TCP port and UDP 5353 for mDNS.

## Configuration

`directories` selects the platform config directory: typically
`~/.config/meshdrop` on Linux, `%APPDATA%\meshdrop` on Windows, and
`~/Library/Application Support/org.meshdrop.meshdrop` on macOS. Use
`--state-dir` or `--config` to override. Missing TOML uses defaults; unknown keys
and invalid values report the configuration filename.

```toml
receive_directory = "received"
port = 9843
allowed_networks = ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16", "100.64.0.0/10", "169.254.0.0/16", "127.0.0.0/8", "::1/128", "fc00::/7", "fe80::/10"]
providers = ["mdns", "manual", "peer-list"]
invitation_timeout_seconds = 60
idle_timeout_seconds = 60
name = "my-laptop"
```

Add `"tailscale"` to providers for optional local `tailscale status --json`
discovery. Missing tools return a reason. Manual candidates can be passed to
`devices host:9843`. Discovery groups successful probes by TLS fingerprint and preserves all observed
addresses. Unresolved candidates fall back to address grouping. Discovery does
not automatically trust candidates. Peer-list
requires reachable, already paired devices. Provider failures are diagnostics on stderr; usable devices remain on stdout.
Invitation timeout limits confirmation waiting; idle timeout limits gaps between
received chunks and accepted uploads that never start. Sustained transfers have
no total-duration limit. The old `timeout_seconds` key is rejected; replace it
with the two explicit keys above. Multi-file completion is per
file, so successfully completed files remain if a later file fails.

## Contribute and verify

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# Rootless Podman by default; CONTAINER_ENGINE=docker selects Docker:
./tests/docker/run.sh
./tests/wine/run.sh
```

Container scripts download images and build dependencies inside containers. Wine
and MinGW are never installed on the host. The three-peer scenario checks real
multicast discovery, explicit pairing, bidirectional transfer hashes and rejection.
The Windows suite runs core and CLI tests in Wine; Linux-only `/proc` VmHWM
measurement is excluded by target configuration. Real Windows, macOS and multicast
LAN behavior still need the [acceptance checklist](docs/acceptance.md). CI verifies
fmt, clippy and tests on Ubuntu, Windows and macOS after a future authorized push.

For macOS, this stage supplies source/CLI only. A locally built unsigned CLI can
be run from Terminal. Future unsigned app bundles will require Finder →
right-click → Open on first launch; that UI is outside this stage.

See [protocol](docs/protocol.md), [progress](progress.md), and
[feature list](feature_list.json). Tauri and Android are planned next; iOS,
Bluetooth and Taildrop compatibility are outside scope.
