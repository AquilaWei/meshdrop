# Changelog

## 0.2.0 - 2026-10-27

- Add Linux x86_64 RPM, universal macOS DMG and release-signed Android arm64 APK installers.
- Verify packaged app startup and publish checksums with installer downloads through a gated release workflow.
- Document checksum verification, macOS Gatekeeper approval and Android debug-to-release migration.
- Keep manual release checks unpublished and clearly label their temporary-key installers as not for distribution.

## 0.1.7 - 2026-10-27

PATCH test build; physical-device transfer acceptance is pending.

- Search at launch, on refresh and every 30 seconds in the visible foreground; keep Android receiving in the background.
- Reduce idle work by retaining unchanged device tiles and QR codes.
- Pair first, add files through ＋ over several picks, then send all selected files in one invitation. Empty selections show an accessible hint.
- Replace grouped Settings with one list; keep port, network restrictions and timeouts in config.toml.
- Show readable file sizes and place the 44px cancel control clear of the progress ring.

## 0.1.6 - 2026-10-24

PATCH test build; physical-device acceptance is pending.

- Show nearby devices as avatars with transfer progress and cancellation.
- Keep search controls stable during background updates and follow light/dark system appearance.
- Organize Settings into My Device, Add Device, Receiving, Network and Advanced.
- Remove individual selected files, select desktop files by dragging into the window, and reveal completed transfers in their folders.
- Improve invitation sheets, keyboard navigation, reduced motion and accessible control labels.

## 0.1.5 - 2026-10-23

PATCH test build; full physical-device transfer acceptance is pending.

- Save desktop and CLI receives to the system Downloads folder by default; resolve configured relative folders beside the configuration file.
- Publish verified Android 10+ receives in public Download without broad storage permission; show each saved location and keep received-file sharing available. Android 9 and below retain private storage.
- Accept one or multiple files from Android’s system share sheet, including when meshdrop is already open.
- Send by tapping a device, opening the file chooser first when needed. Unpaired devices invite automatically after both users confirm the code; each transfer keeps its own status and cancellation.

## 0.1.4 - 2026-10-23

PATCH test build; physical-device acceptance is pending.

- Simplify sending to three steps: search devices, choose files, and send with per-device results and cancellation.
- Show only confirmed meshdrop devices; keep unresolved, offline and non-meshdrop candidates in discovery diagnostics.
- Confirm matching pairing codes during Send, then continue the invitation automatically; remembered devices skip pairing.
- Present incoming pairing requests and file invitations as dialogs.
- Keep device information, QR, manual addresses, diagnostics, settings and Android received-file actions in More.

## 0.1.3 - 2026-10-22

PATCH test build; physical-device acceptance has not been performed.

- Fix Android background reception not starting when notification permission was already granted.
- Android transfers are now verified end-to-end on the emulator: explicit pairing, both directions with SHA-256, rejection, background decision and Stop. Physical-device acceptance is still pending.
- Use swangle for reliable emulator startup and provide a checksum-pinned container fallback; completed files can be shared through Android's native chooser.
- Fix the Android CI build after removal of a legacy SDK package; desktop and Android hosted builds are verified.

## 0.1.2 - 2026-10-22

PATCH test build; physical-device acceptance has not been performed.

- Add the Android app with private receive storage and no broad storage permission.
- Show the matching test-build version in Android app information.
- Keep reception active using an ongoing notification with Stop receiving and Wi-Fi multicast support.
- Show notification permission denial; invitations always require explicit acceptance.
- Scan meshdrop QR codes with the camera and retain manual input when permission is denied.
- Select documents through Android's picker and Share completed received files through the native chooser.
- Explain desktop-only features on Android and add a debug APK CI build.
- Android emulator E2E is blocked by a native emulator SIGSEGV before app installation; camera, notification appearance, Doze/OEM behavior and native interop still require acceptance.

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
