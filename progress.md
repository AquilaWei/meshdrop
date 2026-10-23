# meshdrop progress

Stages 1 and 2 are accepted. Stage 3 Android emulator interoperability now passes
on the host and in a pinned container. The public repository is
AquilaWei/meshdrop; master contains only the new meshdrop history. Publication and
main-worktree fast-forward are authorized. No tag or release has been created.

Implemented: Rust workspace and CLI; strict portable config; private-network
allowlist including mapped IPv6; hidden, verified, collision-safe receive sink;
self-signed identities and persistent explicit trust; mutually authenticated
pinned HTTPS; channel-based pairing/file decisions; bounded streaming/progress;
multiple files; both-side cancellation and disconnect cleanup; mDNS, Tailscale,
peer-list and manual/QR discovery; Linux/Windows/macOS verification CI.

Automated results: Linux workspace 76 tests; three-container discovery/pairing/
transfer/rejection 4 scenarios; Windows target in Wine 53 tests. The Linux-only
VmHWM and Unix-only SIGTERM tests are excluded from the Windows target, with
reasons in `tests/wine/README.md`. Exact commands and results are recorded in
`docs/verification.md`. Hosted Ubuntu, Windows, macOS and Android debug build jobs have passed.

Design choices: TLS certificate fingerprints are identity, advertisements never
establish trust, and TLS signatures are always verified even during bootstrap.
Both users confirm pairing; automatic file acceptance does not bypass pairing.
The receiver delegates decisions through bounded channels. Files publish only
after size/hash verification with no-clobber persistence. Trust is reloaded on
request so a separate CLI pairing process is visible to a running receiver.

Review corrections: invitation waiting and upload inactivity have separate
60-second defaults. A sustained upload has no aggregate deadline. Expiry ignores
active uploads and cleans pending sessions; disconnect cleanup also removes the
session. Discovery reports devices and per-provider/per-peer diagnostics together,
probes at most 16 endpoints concurrently, and groups observed certificate
fingerprints with multiple addresses. Tailscale JSON parsing is pure and tested;
its executable is injectable and offline peers skip probing. Single accept/task
failures are logged without stopping the listener. Blocking receive/trust I/O
runs on the blocking pool with one awaited chunk at a time. No-clobber collision
search stops after 10,000 candidates. Each review item has a separate fix commit
and a regression demonstrated failing before the fix; see the verification table.

Limits: multi-file offers commit files individually; a later failure does not
roll back completed files. Unresolved discovery candidates still group by address.
Missing providers and offline known peers become diagnostics, not global errors. Trust files are atomic but concurrent pairing commands
against the same state directory are not coordinated across processes; perform
pairing sequentially. Abrupt power failure cleanup is not guaranteed. There is
no resumable transfer protocol in this stage.

Stage 2 adds the Tauri desktop shell with static HTML/CSS/JS, CSP and text-only
untrusted names. A bounded message-driven Rust controller runs core reception and
mDNS in the same process, handles explicit pairing and transfer decisions,
streams progress/cancellation, posts native notifications and saves strict TOML
settings atomically. Seventeen controller/QR and four permission tests and seven real WebDriver scenarios
exercise the app with CLI peers. Windows app compilation is checked separately
from the 53 Wine core/CLI tests; WebView2 runtime remains native-only.

Stage 3 adds the generated Android project (minSdk 24, targetSdk 36 unchanged),
app-private receive storage, a dataSync foreground service with Stop action and
MulticastLock, explicit notification permission handling, native FileProvider
sharing of completed files and SAF import for sending. The official barcode
scanner handles camera QR; manual addresses remain available. Android reports
its platform and disables desktop-only Tailscale enumeration/folder opening.

Verified: Android x86_64 debug APK builds, 8 Kotlin JVM safety/lifecycle tests,
and 76 workspace Rust tests. Docker has 4 scenarios; Wine has 53 tests plus the
Windows desktop check; desktop WebDriver has 7 scenarios plus 76 Rust tests.
The Android runner passed all 8 interoperability scenarios on both host and
pinned-container Emulator 37.1.11 using swangle. Legacy SwiftShader GLES JIT
execution caused SEGV_ACCERR; swangle avoids it. An already-granted notification
permission hang was fixed with four red-first regressions. No webview capability
was added. Reproduction and the checksum are in `tests/android/README.md`.

Remaining physical acceptance: Wi-Fi mDNS, Doze/battery/OEM restrictions, native
notification appearance/actions and permission denial, camera QR, SAF selection,
and Fedora/Windows/macOS interoperability. Desktop native checks remain pending.
Private files are removed on uninstall; selected documents are copied into app
cache. Android dataSync lifetime is limited by OS policy; reopen after a stop.
