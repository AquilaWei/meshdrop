# Desktop WebDriver verification

Run from the repository root:

```sh
./tests/ui/run.sh
```

Podman (or `CONTAINER_ENGINE=docker`) builds an isolated image containing GTK,
WebKitGTK, WebKitWebDriver, tauri-driver, Xvfb and Dunst. No host packages are
installed. The runner checks formatting, Clippy and all workspace Rust tests,
then drives the real desktop app against a separate CLI process.

Sixteen scenarios cover stable spinning refresh with no polling mutations or focus loss, background reception and automatic mDNS discovery, tap-to-send
with no selected files, pairing with automatic invitation,
paired sends without another code prompt, app-to-CLI transfer with 100% progress and SHA-256 verification, CLI-to-app
acceptance, verified storage and saved-location display, rejection cleanup, untrusted text rendering,
unresolved/non-meshdrop candidate diagnostics, strict TOML errors that preserve the saved configuration,
Settings navigation/saving, keyboard activation, modal Tab/Escape behavior, removing
one selected file, and the native Tauri drag-drop event. The drop test supplies
native-event paths; physical OS drag gestures remain a manual acceptance check.

The runner also verifies native light/dark preferences and exports 24 deterministic
preview screenshots to `/tmp/meshdrop-016-screens/` (six states, two widths, two
themes). Preview fixtures demonstrate appearance; they do not replace real TLS
transfer checks.

WebDriver cannot operate GTK's native file chooser. The scenario supplies the
chooser result through the existing `desktop_request` / `select_files` command
using WebDriver execute-script. Production picker behavior is unchanged; no
test environment switch or alternate transfer implementation is added. Native
file selection remains a physical acceptance item.

Dunst provides a notification service in the container. GNOME/KDE notification
presentation, Windows WebView2 and macOS UI integration require the checks in
[the acceptance checklist](../../docs/acceptance.md).
