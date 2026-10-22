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
| Linux workspace | 53 passed, 0 failed: CLI processes 5, transport 21, discovery 11, identity 4, storage/config/network 12 |
| Rootless Podman, three Ubuntu containers | 4 scenarios passed, 0 failed: mutual mDNS discovery, explicit pairing, bidirectional SHA-256 transfer, rejection without content |
| Windows GNU target, containerized Wine | 51 passed, 0 failed: CLI processes 3, transport 21, discovery 11, identity 4, storage/config/network 12 |

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
online precedence, versioned QR parsing, Tailscale IPv4/IPv6/offline/no-Peer
fixtures and missing injected executables. A real HTTPS server plus an unreachable
peer exercises partial peer-list retrieval. Actual sockets verify concurrent probe
startup and the 16-connection ceiling; a real identity exposed at an IP and a
hostname verifies fingerprint deduplication and retention of both addresses.
Actual mDNS browsing plus TLS info probes is exercised by containers.

See `tests/wine/README.md` for the two platform-specific tests not compiled on
Windows. Wine may print headless XDG runtime diagnostics during initialization;
all Windows test executables and CLI subprocesses completed successfully.

Unverified: hosted Windows/macOS CI and physical environments listed in
`docs/acceptance.md`. Tailscale parsing and missing-command behavior are tested with fixed fixtures
and an injected path; enrollment and connectivity on an actual tailnet remain
unverified.

## Review regression evidence

Each row was run before implementing that item's behavioral fix and failed as
shown, then passed after the fix. Where injection was needed, extraction retained
the original fail-fast or synchronous behavior before running the red test. These
are recorded observations, not claims that missing new APIs compiled on the old
HEAD. Additional coverage tests for already-correct cases also pass.

| Review item / commit | Observed before fix | Passing regression after fix |
|---|---|---|
| 1 / `2206c66` | Slow continuous upload returned HTTP 400 instead of 200; idle response lacked an idle-timeout reason | Four chunks spaced 400 ms apart complete despite a 1-second timeout; a stalled stream reports idle timeout and leaves no partial file |
| 2 / `0894408` | A failing provider made discovery return an error and lose another provider's result | Result and diagnostic coexist; CLI prints devices and reports problems separately |
| 3 / `24f9ef3` | Extracted old parsing marked an offline fixture online; missing-tool error omitted the injected path; old fail-fast PeerList lost the real server's result after an unreachable peer | Fixed IPv4/IPv6/no-Peer/offline fixtures, injected missing executable path, and real online-plus-offline PeerList pass |
| 4 / `72ba198` | Three TCP probes could not start within one second because the first awaited its two-second TLS timeout | Probes start concurrently and no seventeenth connection starts while sixteen are held open |
| 5 / `0c7034c` | One TLS certificate at an IP and hostname produced two devices | One device retains both addresses and the observed fingerprint |
| 6 / `84f5374` | Injected ConnectionAborted and a panicked connection task each failed the loop | Subsequent connections are processed and failures are logged during normal operation and drain |
| 7 / `ce3e979` | Synchronous storage blocked the single-thread executor, preventing another async task from releasing its gate | Blocking work yields the executor; storage errors propagate; cancellation and disconnect cleanup still pass |
| 8 / `d11e8a5` | With 10,000 occupied candidates the old sink created candidate 10,001 | Collision exhaustion returns an error, preserves existing content and removes the temporary file |

Verification commands above completed with exit code 0. Container execution used
rootless Podman. Windows excluded only the same two target-specific tests listed
in `tests/wine/README.md`; none of the new regressions were excluded.
