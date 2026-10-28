# Physical-device acceptance (not yet performed)

Only checks not established by local/Wine/container simulation are listed here.

| Environment | Steps | Expected result |
|---|---|---|
| Real Windows | Build with stable Rust, run `receive`, pair with Linux, transfer Unicode filenames and an existing filename in both directions. Repeat on an NTFS destination under normal user permissions; press console Ctrl-C during reception. | Prompts and progress work in the native terminal; bytes match SHA-256; existing files remain; no partial files remain on cancellation; console Ctrl-C exits cleanly. |
| Real macOS | Build locally, launch from Terminal, pair and transfer with Linux/Windows; test sleep/wake and network changes. | Identity survives restart, pairing remains valid, transfers fail clearly on disconnect and resume only through a new invitation. |
| Real Wi-Fi / Ethernet LAN | Put three hosts on one LAN, permit TCP 9843 and UDP 5353, run `receive`, then `devices`; repeat with an access point that isolates clients. | Reachable peers appear; isolated networks do not claim connectivity. Router multicast filtering can prevent discovery; manual addresses remain usable where routing permits. |
| Tailscale / ZeroTier / WireGuard deployment | Enable the optional Tailscale provider on a Tailscale host; use manual addresses for other VPNs. Pair and transfer through the actual VPN routing/firewall policy. | Tailscale enumerates and probes actual peers; other routed private addresses work independently of Tailscale; unreachable peers appear only in CLI discovery diagnostics. |
| OS shutdown and storage behavior | Cancel transfers during sleep, unplug an external destination drive, and retry on its actual filesystem. | No existing file is replaced; errors are visible; temporary files are removed where the filesystem remains accessible. Abrupt power loss may leave hidden temporary files and is not an orderly cancellation. |

## Desktop integration

| Environment | Steps | Expected result |
|---|---|---|
| Fedora GNOME and KDE (test both) | Start the app, pair a CLI peer, minimize the app and send an offer. Accept in the app; repeat with notifications disabled in Settings. | Native notification appears with notifications enabled; dialog shows sender, filename and size; disabled notifications leave the dialog functional. No implicit acceptance. |
| Native Windows | Launch the app with WebView2 installed; allow its firewall prompt on private networks only. Pair and transfer both ways, select multiple files in the native chooser, cancel, and open the receive folder. Test notification permissions. | Interface, picker, progress and notifications work; private-network peers connect; cancellation removes partial files and folder opening uses Explorer. Public sources remain rejected. |
| Native macOS | Open an unsigned bundle using Finder right-click → Open; if blocked use System Settings → Privacy & Security → Open Anyway. Allow notifications, pair and transfer in both directions, select files and open the destination. | App opens after explicit approval; native layout, picker, notifications and Finder integration work; no signing is claimed. |
| Desktop lifecycle | Close the window during a slow transfer, reopen, then change settings and restart. | Receiver and advertisement stop cleanly; partial files are removed; saved port/folder/provider settings apply after restart and identity persists. |

Container WebDriver injects native chooser results; actual chooser interaction,
GNOME/KDE presentation, Windows WebView2 and macOS integration are not physically
verified. Android emulator coverage establishes basic service lifecycle, explicit
pairing, bidirectional hashes, rejection and FileProvider chooser dispatch.

## Android devices (not yet performed)

| Check | Steps | Expected result |
|---|---|---|
| Real Wi-Fi mDNS | Put a phone and Fedora/Windows/macOS on the same Wi-Fi; allow multicast, start reception and refresh peers. Repeat with client isolation enabled. | Reachable peers show name/platform/online state. Isolation reports unreachable peers; manual addresses work where routed. No advertisement establishes trust. |
| Native notification actions and appearance | Send an invitation with the phone locked, tap its notification, then use Stop receiving in the ongoing notification. | Notification appears correctly; tap returns to the pending decision; the native Stop action stops reception. Basic Home reception and in-app Stop are already emulator-tested. |
| Doze / battery | Test with screen off and Android Doze; test both optimized and explicitly unrestricted battery settings. Leave receiving beyond the OS dataSync runtime allowance. | Document delays and system restrictions. App reports stopped reception rather than claiming indefinite reception; reopen to receive again after the OS timeout. |
| Notification permission | On Android 13+, deny POST_NOTIFICATIONS, send an offer while app is visible, then enable permission in system settings and restart. | Denial is explained; explicit in-app decisions still work. After permission is enabled, native invitations and ongoing notification are visible. |
| Camera QR permission | Open Settings → Add Device, scan a real meshdrop QR from another device, tap its avatar and compare codes to pair; then press ＋ to choose files and tap the connected avatar. Repeat with a foreign QR, denied camera permission and a changed certificate. | Only valid versioned meshdrop QR is accepted; both users confirm codes; changed pins are rejected. Denied permission keeps manual/pasted input usable. |
| Native documents / received sharing | Pick multiple files through the Android document provider and send them. Receive two identically named files, then Share each from Received files. Try an unfinished transfer. | Selected SAF content matches SHA-256 at destination; completed files never overwrite one another; sharing grants temporary read access to completed public Download entries on API 29+, or private receive files on older Android. Partial files and private identities cannot be shared. |
| OEM background restrictions | Repeat Home, screen-off and notification tests on the user's phone brand, including its battery/auto-start controls. | Record settings required by that manufacturer; do not promise a foreground service bypasses Doze or forced process termination. |
| Cross-platform interoperability | Pair independently with Fedora, Windows and macOS; transfer Unicode filenames, multiple files, collisions and a large file in both directions; cancel on each side. | Explicit pairing/accept/reject, progress, cancellation cleanup and hash verification work on native devices. |

The emulator suite includes shared intents and public Download publication.
These checks retain only device, OS presentation and real-network behavior not
established by those simulations; see `tests/android/README.md`.

## Simplified shared UI (physical checks pending)

On Fedora and the phone, open the app and verify Devices, Files and Transfers
appear on the main page and searching starts automatically. Include an offline Tailscale peer and a
non-meshdrop host: neither should appear in the device list; the CLI reports discovery diagnostics.
Tap an unpaired meshdrop peer with no files selected: compare codes and pair,
without a chooser or invitation. Verify the avatar becomes Connected. Press ＋
several times to accumulate files, remove one, then tap the connected peer: one
invitation must contain every remaining file. A connected avatar with no files
must announce the one-line plus-first hint and send nothing. Fingerprint mismatch
avatars must be disabled. Reject, time out and cancel transfers independently.
These changed flows have automated scenarios but are not claimed physically tested.

## Receive locations and Android share target (0.1.5)

Automated checks cover the platform Downloads default, config-relative folder priority,
verified-only publication, naming collisions, tap-to-send, automatic invitations after
pairing, saved locations, shared intents and public Download hashes.

Physical checks still required:

- Launch desktop/CLI from different working directories on native Windows/macOS;
  check the OS Downloads folder and an explicit configured destination.
- On Android 10+, receive colliding names from multiple devices; check Download in
  the system file manager and Share each received entry. Cancel a large receive.
  Existing content must remain, with no pending/partial public entries.
- From the phone’s actual gallery and document providers, Share one and multiple
  files into meshdrop, both closed and already open; tap a peer and verify hashes.
- On Android 9 or below, check private storage and completed-file Share. Public
  Downloads is intentionally unavailable there.

The phone install/startup smoke does not establish these physical transfer checks.

## Apple-style interface (0.1.6)

The main screen uses avatars with transfer rings, removable file chips and
collapsible Recent history. Refresh remains keyboard-focusable during searches.
The grouped Settings introduced in 0.1.6 is replaced by the flat list below in 0.1.7. Name, folder,
port and network changes require restart; notifications apply immediately.
Light/dark themes follow the system; reduced motion replaces the search rotation
with a static searching indicator. Every icon-only action has an accessible name
and tooltip. Native dialogs trap focus; Escape cancels pairing or declines incoming requests.

## Foreground discovery and connect-first flow (0.1.7)

Search only at launch, on ↻, and every 30 seconds in the visible foreground.
Minimize, switch apps and hide the page: no new discovery request may start.
Android must still receive and retain pending invitations in the background.
On return, a previously elapsed 30-second interval may trigger a search.
Unchanged snapshots retain the same device DOM; address-only metadata updates
also preserve tiles. QR encoding and canvas drawing happen only when their
payload/matrix changes. Performance evidence includes actual WebKit poll timings,
DOM mutations, search counts and actor/QR generation timings.

Settings contains one level only. Restart labels apply to name, folder and
providers; notifications apply immediately. Android shows Download and Stop
receiving; sharing completed receives remains in Recent. Port, allowed networks
and timeouts remain in config.toml with unchanged defaults. Read README before
editing them by hand; diagnostics remain available through the CLI.

### Old location → new location

| 0.1.6 location | 0.1.7 location |
| --- | --- |
| Settings → My Device → name, fingerprint, QR | Settings → device name, fingerprint, QR |
| Settings → Add Device → address / QR scan | Settings → Add Device / Android Scan QR |
| Settings → Receiving → receive folder / open folder | Settings → Receive folder / desktop Open receive folder; Android Download |
| Settings → Receiving → notifications | Settings → System notifications |
| Settings → Receiving → Android Stop receiving | Settings → Stop receiving |
| Settings → Receiving → Received files / Share | Recent → completed receive → Share |
| Settings → Network → discovery providers | Settings → Discovery providers |
| Settings → Receiving → Port | config.toml → port (restart) |
| Settings → Network → allowed networks | config.toml → allowed_networks (restart) |
| Settings → Network → invite / idle timeouts | config.toml → invitation_timeout_seconds / idle_timeout_seconds (restart) |
| Settings → Advanced → TOML editor | External text editor → config.toml |
| Settings → Advanced → Diagnostics | CLI devices → stderr |

### Verification limits

The 0.1.7 evidence is recorded in `/tmp/meshdrop-017-final-evidence.md` and the
screenshots in `/tmp/meshdrop-017-screens`. Screenshots use deterministic fixtures
in the real webview; transport checks use real peers. The required regressions
are shown failing on 0.1.6 before the production changes, then passing on 0.1.7.

Physical checks still required: native desktop chooser interaction, screen-reader
speech, macOS/Windows presentation, real-phone camera/notification/transfer and
OEM background behavior. Verify desktop/mobile light/dark states for main,
searching, connected, plus-first hint, invite modal, pairing and flat Settings.
During a send the cancel target must be at least 44×44px and entirely outside the
progress ring. A phone install/start/version/crash smoke does not establish these
physical transfer checks. No 0.2.0 tag or release is authorized by this patch.

## Installer acceptance before a release tag

- Build the x86_64 RPM and inspect it with `rpm -qip`, `rpm -qlp` and
  `rpm -i --test`. The workflow extracts it and checks its window under Xvfb.
  A user performs the privileged Fedora installation:
  `sudo dnf install ./meshdrop-0.2.0-x86_64.rpm`, then `meshdrop-app`.
- The macOS dry-run runner must mount the universal DMG, verify both architectures,
  launch the bundled app, confirm it stays running, then terminate it and unmount.
  Gatekeeper approval, native notifications and file pickers remain physical checks.
- Verify the release-signed arm64 APK using aapt and apksigner, then install and
  launch it on an isolated test emulator using `tests/android` infrastructure.
  The Google APIs x86_64 image supplies ARM64 native translation; the same smoke
  check runs on hosted Linux. Never use the user's AVD or phone.
- Check the manual release workflow and all four ordinary push jobs at final HEAD.
  The dry run must not create a Release. No tag or publication is authorized here.
- Test upgrading release APKs with the same key separately; moving from debug to
  release requires uninstalling (deleting app data) and pairing again. Temporary
  dry-run keys cannot establish upgrade compatibility.
