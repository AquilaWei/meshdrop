# Changelog

## Desktop stage-two development

- Add a Tauri 2 desktop app using static HTML, CSS and JavaScript.
- Receive in the background, discover peers and show provider diagnostics and identity QR.
- Confirm pairing on both devices; send to multiple paired devices with progress and cancellation.
- Show incoming dialogs and system notifications; save strict TOML settings and notification preferences.
- Add twelve Rust desktop state tests, seven Xvfb/WebDriver scenarios and Windows app compile checks.
- Document native platform acceptance and unsigned macOS first launch.

## Review corrections

- Separate invitation waiting from upload idle timeouts so active large transfers continue.
- Preserve device results when providers or known peers fail, with diagnostics on stderr.
- Probe at most sixteen endpoints concurrently and group devices by certificate identity while retaining addresses.
- Handle individual accept/connection failures without stopping reception.
- Move receive and trust-store disk work off the async executor; bound filename collision retries.
- Add Tailscale fixtures, real partial peer-list tests and regression verification for every review item.

## Initial stage-one development

- Create the meshdrop Rust workspace and cross-platform verification CI.
- Add private-network policy, safe collision-free downloads and strict TOML settings.
- Persist device certificates and explicitly confirmed fingerprint trust.
- Add HTTPS file invitations, bounded streaming, progress, cancellation and integrity checks.
- Add LAN, Tailscale, peer-list and manual discovery, plus the command-line frontend.
- Add process-level transfer tests and container verification scripts.

Version is inherited exclusively from the root Cargo workspace manifest.

- Desktop preparation: invitations retain the sender display name while certificate pins remain authoritative.
