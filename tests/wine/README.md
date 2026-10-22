# Windows tests in Wine

Run `./tests/wine/run.sh` from the checkout. It builds MinGW and Wine inside a
container, copies source without Git metadata/build artifacts/private directories,
and runs `cargo test --workspace --exclude meshdrop-app --locked --target x86_64-pc-windows-gnu` with the
Wine Cargo runner. The script propagates the container's exit code.

The Windows target runs the full core suite plus the CLI identity, rejection and
bidirectional process tests. Two target-specific tests are not compiled:

- `large_transfer_keeps_receiver_child_below_96_mib`: Linux-only `/proc/<child>/status`
  VmHWM measurement has no equivalent in this Wine/Windows executable. Linux runs
  a real 128-MiB transfer and checks the receiver child's peak stays below 96 MiB.
- `sigterm_stops_interactive_receiver_cleanly`: SIGTERM and the Unix `kill` utility
  are not Windows process APIs. Linux tests this signal path. Native Windows
  console Ctrl-C behavior remains a physical-device check.

Wine cannot establish native Windows firewall, antivirus, console or NTFS ACL
behavior. See `docs/acceptance.md`; no host Wine installation is needed.

## Desktop app compile check

The same container also runs `tests/wine/inside.sh`, which after the core and CLI
tests executes `cargo check -p meshdrop-app --locked --target x86_64-pc-windows-gnu`.
This checks the Tauri 2 desktop crate for the Windows target with MinGW. It does not
run the app: WebView2 cannot start under Wine, so the Windows window, notifications,
firewall prompt and native file dialog remain physical-device checks in
`docs/acceptance.md`.
