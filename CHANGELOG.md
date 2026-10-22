# Changelog

## 0.1.1 - 2026-10-22

PATCH test build; physical-device acceptance has not been performed.

- Add a Tauri 2 desktop app with device lists, discovery diagnostics and identity QR.
- Pair using QR contents or matching short codes with explicit confirmation on both devices.
- Invite multiple paired devices to receive multiple files; show incoming notifications and accept/reject dialogs.
- Show progress and cancellation on both sides, with completion after SHA-256 verification.
- Configure the receive folder, port, networks, discovery providers, invitation/idle timeouts and notifications in Settings.
- Show the sender's display name in invitations while keeping certificate fingerprints authoritative.
- Windows/macOS interfaces, native file selection and notification appearance await physical-device acceptance.

## 0.1.0

### Review corrections

- Separate invitation waiting from upload idle timeouts so active large transfers continue.
- Preserve device results when providers or known peers fail, with diagnostics on stderr.
- Probe at most sixteen endpoints concurrently and group devices by certificate identity while retaining addresses.
- Handle individual accept/connection failures without stopping reception.
- Move receive and trust-store disk work off the async executor; bound filename collision retries.
- Add Tailscale fixtures, real partial peer-list tests and regression verification for every review item.

### Initial stage-one development

- Create the meshdrop Rust workspace and cross-platform verification CI.
- Add private-network policy, safe collision-free downloads and strict TOML settings.
- Persist device certificates and explicitly confirmed fingerprint trust.
- Add HTTPS file invitations, bounded streaming, progress, cancellation and integrity checks.
- Add LAN, Tailscale, peer-list and manual discovery, plus the command-line frontend.
- Add process-level transfer tests and container verification scripts.
