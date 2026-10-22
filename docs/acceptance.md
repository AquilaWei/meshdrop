# Physical-device acceptance (not yet performed)

Only checks not established by local/Wine/container simulation are listed here.

| Environment | Steps | Expected result |
|---|---|---|
| Real Windows | Build with stable Rust, run `receive`, pair with Linux, transfer Unicode filenames and an existing filename in both directions. Repeat on an NTFS destination under normal user permissions; press console Ctrl-C during reception. | Prompts and progress work in the native terminal; bytes match SHA-256; existing files remain; no partial files remain on cancellation; console Ctrl-C exits cleanly. |
| Real macOS | Build locally, launch from Terminal, pair and transfer with Linux/Windows; test sleep/wake and network changes. | Identity survives restart, pairing remains valid, transfers fail clearly on disconnect and resume only through a new invitation. |
| Real Wi-Fi / Ethernet LAN | Put three hosts on one LAN, permit TCP 9843 and UDP 5353, run `receive`, then `devices`; repeat with an access point that isolates clients. | Reachable peers appear; isolated networks do not claim connectivity. Router multicast filtering can prevent discovery; manual addresses remain usable where routing permits. |
| Tailscale / ZeroTier / WireGuard deployment | Enable the optional Tailscale provider on a Tailscale host; use manual addresses for other VPNs. Pair and transfer through the actual VPN routing/firewall policy. | Tailscale enumerates and probes actual peers; other routed private addresses work independently of Tailscale; unreachable peers show offline/errors. |
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
verified. Android foreground service and interoperability are the next stage.
