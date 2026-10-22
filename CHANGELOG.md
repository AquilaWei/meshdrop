# Changelog

## Review corrections

- Separate invitation waiting from upload idle timeouts so active large transfers continue.

## Initial stage-one development

- Create the meshdrop Rust workspace and cross-platform verification CI.
- Add private-network policy, safe collision-free downloads and strict TOML settings.
- Persist device certificates and explicitly confirmed fingerprint trust.
- Add HTTPS file invitations, bounded streaming, progress, cancellation and integrity checks.
- Add LAN, Tailscale, peer-list and manual discovery, plus the command-line frontend.
- Add process-level transfer tests and container verification scripts.

Version is inherited exclusively from the root Cargo workspace manifest.
