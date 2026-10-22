# Physical-device acceptance (not yet performed)

Only checks not established by local/Wine/container simulation are listed here.

| Environment | Steps | Expected result |
|---|---|---|
| Real Windows | Build with stable Rust, run `receive`, pair with Linux, transfer Unicode filenames and an existing filename in both directions. Repeat on an NTFS destination under normal user permissions; press console Ctrl-C during reception. | Prompts and progress work in the native terminal; bytes match SHA-256; existing files remain; no partial files remain on cancellation; console Ctrl-C exits cleanly. |
| Real macOS | Build locally, launch from Terminal, pair and transfer with Linux/Windows; test sleep/wake and network changes. | Identity survives restart, pairing remains valid, transfers fail clearly on disconnect and resume only through a new invitation. |
| Real Wi-Fi / Ethernet LAN | Put three hosts on one LAN, permit TCP 9843 and UDP 5353, run `receive`, then `devices`; repeat with an access point that isolates clients. | Reachable peers appear; isolated networks do not claim connectivity. Router multicast filtering can prevent discovery; manual addresses remain usable where routing permits. |
| Tailscale / ZeroTier / WireGuard deployment | Enable the optional Tailscale provider on a Tailscale host; use manual addresses for other VPNs. Pair and transfer through the actual VPN routing/firewall policy. | Tailscale enumerates and probes actual peers; other routed private addresses work independently of Tailscale; unreachable peers show offline/errors. |
| OS shutdown and storage behavior | Cancel transfers during sleep, unplug an external destination drive, and retry on its actual filesystem. | No existing file is replaced; errors are visible; temporary files are removed where the filesystem remains accessible. Abrupt power loss may leave hidden temporary files and is not an orderly cancellation. |

Next stages: Tauri UI and native notifications, then Android foreground service
and Android interoperability. Their UI/permission/battery behavior is not part of
this CLI stage. Future unsigned macOS app bundles need a documented first-launch
Finder Open procedure; there is no app bundle to approve in this stage.
