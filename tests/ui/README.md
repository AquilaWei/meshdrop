# Desktop WebDriver verification

Run `./tests/ui/run.sh` from the repository root. The existing Podman image (or
`CONTAINER_ENGINE=docker`) checks fmt, Clippy, workspace tests and the real Tauri
app under Xvfb/WebKitWebDriver against a separate CLI peer.

`regression.py` proves no automatic search before 30 seconds, no search over 31
hidden seconds and preservation of device nodes after nonvisual address changes.
It profiles 100 real snapshot round trips, DOM mutations and QR redraw counts.
`cargo test -p meshdrop-app profile_unchanged_snapshots -- --ignored --nocapture`
separately measures actor snapshots and QR encoding without browser overhead.

The UI scenario covers every avatar case: unpaired with/without files pairs
only; paired without files announces the plus-first hint; paired with multiple
files issues exactly one invitation; fingerprint mismatches disable activation.
Separate plus picks append files and each chip can be removed. Real TLS transfers
check both files and SHA-256, rejection, saved locations, unsafe text rendering,
flat Settings/config preservation, keyboard activation and modal focus/Escape.
Native drag/drop no longer selects files.

WebDriver supplies only the native chooser result through `append_files` because
it cannot operate GTK's chooser. Real native picker presentation remains a
physical check; there is no alternate production transport or test-mode switch.

`screenshots.py` captures 32 deterministic previews at desktop/mobile widths in
native light/dark themes, including main, searching, connected, choose-first,
incoming invitation, pairing, Settings and its lower options. Files are copied to
`/tmp/meshdrop-017-screens`. Every visible button/summary is checked for a 44px
target, icon labels, no horizontal overflow and no cancel/ring overlap. Fixtures
illustrate appearance; the separate scenario establishes real transport behavior.

GNOME/KDE notifications, Windows WebView2, macOS and screen-reader speech require
[physical acceptance](../../docs/acceptance.md).
