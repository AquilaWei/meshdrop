# meshdrop progress

Stage 1 implementation and automated verification are complete. The new orphan
history is on `hb/19-linux-windows-ios-macos-airdro`; no legacy crates or prior
commits were carried into it. No push, tag, release, master update or main-worktree
edit was performed.

Implemented: Rust workspace and CLI; strict portable config; private-network
allowlist including mapped IPv6; hidden, verified, collision-safe receive sink;
self-signed identities and persistent explicit trust; mutually authenticated
pinned HTTPS; channel-based pairing/file decisions; bounded streaming/progress;
multiple files; both-side cancellation and disconnect cleanup; mDNS, Tailscale,
peer-list and manual/QR discovery; Linux/Windows/macOS verification CI.

Automated results: Linux workspace 53 tests; three-container discovery/pairing/
transfer/rejection 4 scenarios; Windows target in Wine 51 tests. The Linux-only
VmHWM and Unix-only SIGTERM tests are excluded from the Windows target, with
reasons in `tests/wine/README.md`. Exact commands and results are recorded in
`docs/verification.md`. Hosted CI has not run because no push was authorized.

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

Next: Tauri interface and notifications, then Android app/foreground service.
No UI, Android, iOS, Bluetooth or Taildrop implementation is included here.
Physical-device steps and expected results are in `docs/acceptance.md`. Real
Windows/macOS terminals, real LAN multicast/VPN routing and storage unplug/sleep
behavior remain unverified. Do not describe them as physically tested.
