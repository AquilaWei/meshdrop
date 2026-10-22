# meshdrop protocol

All endpoints use HTTPS with rustls, HTTP/1.1 and mutual TLS. Each installation
creates an rcgen self-signed certificate. Device IDs are lowercase hexadecimal
SHA-256 fingerprints of certificate DER. TLS handshake signatures are verified;
public CA names and device display names do not grant trust. Every JSON request
or metadata response includes `version: 1`; incompatible versions fail explicitly.

## Bootstrap and trust

Only `/info` and `/pair` are available before pairing. A QR payload is
`meshdrop://host:port#v=1&fp=<64 hexadecimal digits>`. A QR fingerprint is checked
in the TLS handshake before metadata is accepted. With a manual address, both
users compare the six-digit code derived from SHA-256 of the two sorted
fingerprints, joined with `:`. Neither endpoint stores trust before confirmation.
The receiver confirms through the invitation channel; the sender confirms in
its frontend before POSTing `/pair`. Pair requests include the sender's reachable
`address`. Both sides store address-to-fingerprint bindings atomically. Subsequent
connections pin the stored certificate. A changed certificate produces the
explicit error `fingerprint mismatch`; recovery requires deliberate local trust
removal and a new pairing. Never automatically replace a pin.

## API

- `GET /info`: version, display name, platform and certificate fingerprint. The
  client checks the advertised fingerprint against the TLS peer certificate.
- `POST /pair`: version and return address; waits for receiver confirmation.
  Returns receiver info only after acceptance. Rejection and timeout are errors.
- `GET /peers`: paired callers receive known addresses and advertised fingerprints.
  These extend discovery only; they do not grant local trust.
- `POST /offer`: version and `files` array of name, size and SHA-256 hex digest.
  One invitation authorizes 1–128 files. A bounded channel delegates acceptance
  to the frontend. Rejection, channel closure, cancellation or configured timeout
  returns HTTP 400 with a reason; no file bytes have been uploaded.
- Accepted offers return a random transfer ID. `POST /upload/{id}/{index}` streams
  each authorized file once. Ownership is tied to the TLS client fingerprint.
  The client reads at most 64 KiB ahead; receiver disk writes and bounded progress
  channels apply backpressure. The server enforces declared size and SHA-256
  before publishing the hidden `.part` with no-clobber persistence. Collisions
  receive numbered names, never overwrite. HTTP 400 reports integrity failures.
- `POST /cancel/{id}` cancels a sender-owned transfer. The receiver retains the
  invitation's cancellation token and can cancel while uploading. Disconnects,
  cancellation, timeouts and shutdown drop the sink and remove partial files.

Network allowlisting occurs before TLS; IPv4-mapped IPv6 is normalized. Public
sources are denied by default. Metadata bodies are capped at 64 KiB. Uploads
stream without an aggregate memory buffer. Configured transfer timeout bounds
both incomplete sessions and each upload; increase it for large/slow transfers.
Progress indicates locally processed bytes; success is the final HTTP response,
not the last progress event. Multi-file transfers commit each file separately:
a later failure leaves earlier verified, completed files intact.

## Discovery

Providers are independent of transport: DNS-SD `_meshdrop._tcp.local.`, local
`tailscale status --json` plus info probes, authenticated peer lists, and manual
addresses/QR. Results merge by socket address, with online data preferred.
Discovery never implicitly pairs devices. mDNS is limited to a multicast domain;
VPN connectivity does not imply multicast forwarding.
