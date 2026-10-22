# meshdrop

**Private-network file invitations for Linux, Windows and macOS.** No cloud,
public CA or Tailscale account required. The Rust core is shared by the CLI and
Tauri desktop app; Android is planned next. GPL-3.0-or-later; see [LICENSE](LICENSE).

## Try it

Install stable Rust with Cargo, then:

```sh
cargo build -p meshdrop-cli --locked
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
`devices host:9843`. Discovery groups successful probes by TLS fingerprint and
preserves all observed addresses. Unresolved candidates fall back to address
grouping. Discovery does not automatically trust candidates. Peer-list queries
already paired devices; an unreachable peer does not hide other results.
Provider failures are diagnostics on stderr; usable devices remain on stdout.
Probes run concurrently with a limit of sixteen; Tailscale offline peers are
listed without probing.
Invitation timeout limits confirmation waiting; idle timeout limits gaps between
received chunks and accepted uploads that never start. Sustained transfers have
no total-duration limit. The old `timeout_seconds` key is rejected; replace it
with the two explicit keys above. Multi-file completion is per
file, so successfully completed files remain if a later file fails.

## Contribute and verify

Android development uses a user-owned compatible JDK, SDK and NDK; set
`JAVA_HOME`, `ANDROID_HOME` and `NDK_HOME` to those installations. For example,
`JAVA_HOME=$HOME/.local/share/meshdrop-android/jdk-17` can be reused by scripts.
From `crates/meshdrop-app`, run `cargo tauri android build --debug --target x86_64`.
No host system packages or release signing keys are installed by this project.


```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# Rootless Podman by default; CONTAINER_ENGINE=docker selects Docker:
./tests/docker/run.sh
./tests/wine/run.sh
./tests/ui/run.sh
```

Container scripts download images and build dependencies inside containers. Wine
and MinGW are never installed on the host. The three-peer scenario checks real
multicast discovery, explicit pairing, bidirectional transfer hashes and rejection.
The Windows suite runs core and CLI tests in Wine; Linux-only `/proc` VmHWM
measurement is excluded by target configuration. Real Windows, macOS and multicast
LAN behavior still need the [acceptance checklist](docs/acceptance.md). CI verifies
fmt, clippy and tests on Ubuntu, Windows and macOS after a future authorized push.

The Xvfb/WebDriver suite drives seven desktop scenarios against real CLI peers.
Its chooser-result injection and native UI limits are documented in
[tests/ui/README.md](tests/ui/README.md). Wine also checks the Windows app target;
WebView2 UI execution requires native Windows.

See [protocol](docs/protocol.md), [progress](progress.md), and
[feature list](feature_list.json). Android is planned next; iOS, Bluetooth and
Taildrop compatibility are outside scope.

## Desktop app (Tauri 2)

The desktop shell uses static HTML/CSS/JS without Node, a frontend framework or a
bundler. On Linux it needs Tauri's GTK 3 / WebKitGTK 4.1 development libraries;
Windows needs the native Rust toolchain and WebView2, macOS needs Xcode tools.
No host packages are installed by the repository scripts.

```sh
cargo build -p meshdrop-app --locked
cargo run -p meshdrop-app --locked
```

The window starts the core receiver and mDNS advertisement in the same process;
closing it cancels reception and withdraws discovery. Use `MESHDROP_STATE_DIR` to
select an isolated profile; otherwise the CLI and app share the platform config
folder and cannot bind the same port simultaneously.

The app displays provider diagnostics without hiding usable devices. Add an address
or QR payload, compare six-digit codes on both devices, then explicitly accept the
incoming pairing. Select paired devices and files to send; incoming file offers
always require an in-app decision. Device names and filenames render as text.
Settings are saved atomically to the shared `config.toml`; unknown keys include the
file path in the error. `notifications_enabled` toggles desktop notifications
immediately. Restart after changing the receive folder, port, providers, networks
or either timeout. Closing the app stops its background receiver.

On Linux, launch `./target/debug/meshdrop-app`; on Windows, launch
`target\debug\meshdrop-app.exe` (WebView2 must be installed). On macOS, run
`./target/debug/meshdrop-app` from Terminal. For an unsigned `.app` bundle,
open Finder and **right-click → Open** on first launch, then confirm Open.
If Gatekeeper blocks it, go to **System Settings → Privacy & Security → Open
Anyway**, authenticate, and confirm Open. No signing or release is performed here.

Native appearance and notification permissions still need the checks in
[the acceptance checklist](docs/acceptance.md).

## Android app

The Android target shares the same core and static interface. Build a debug APK
using the development command above, then install the generated debug APK with
`adb install -r PATH_TO_DEBUG_APK`, or open it on your device and explicitly allow
installation from that source. Only Gradle's automatic debug key is used.

Received files land in app-private storage without broad storage permission;
uninstalling the app deletes that storage. Native file sharing and background
reception integration are being added in this stage and are not yet verified.
Tailscale enumeration and desktop folder opening are unavailable on Android;
use a manual private VPN address instead. The receive path is shown in Settings.
