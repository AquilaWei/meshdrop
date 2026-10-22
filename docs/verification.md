# Stage-one automated verification

Commands run locally, without a push or hosted CI:

```sh
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
./tests/docker/run.sh
./tests/wine/run.sh
```

| Suite | Actual result |
|---|---|
| fmt / clippy | Pass; clippy warnings denied |
| Linux workspace | 36 passed, 0 failed: CLI processes 5, transport 13, discovery 3, identity 4, storage/config/network 11 |
| Rootless Podman, three Ubuntu containers | 4 scenarios passed, 0 failed: mutual mDNS discovery, explicit pairing, bidirectional SHA-256 transfer, rejection without content |
| Windows GNU target, containerized Wine | 34 passed, 0 failed: CLI processes 3, transport 13, discovery 3, identity 4, storage/config/network 11 |

The Linux process test transfers a sparse 128-MiB source through the real CLI and
checks the receiving child process's own VmHWM is below 96 MiB. This measures the
target process, not the parent test runner. The test fails rather than silently
skipping if `/proc` cannot be read. Other process tests verify actual bytes in
both directions and a rejection that produces no file.

Transport tests cover offer notification, accept/reject/timeout, sender and
receiver cancellation before and during upload, disconnect cleanup, pairing,
changed TLS pins, unpaired senders, multi-file offers and corrupt content.
Storage/config tests cover path stripping, Windows names and characters, UTF-8
length truncation, collision preservation, failed hash cleanup, source policy,
mapped IPv6 and unknown TOML keys with filenames. Provider tests cover merging,
online precedence and versioned QR parsing. Actual mDNS is exercised by containers.

See `tests/wine/README.md` for the two platform-specific tests not compiled on
Windows. Wine may print headless XDG runtime diagnostics during initialization;
all Windows test executables and CLI subprocesses completed successfully.

Unverified: hosted Windows/macOS CI and physical environments listed in
`docs/acceptance.md`. Tailscale integration uses the real CLI adapter but has not
been exercised against an enrolled Tailscale network in this environment.
