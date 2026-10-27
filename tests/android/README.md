# Android emulator verification

Run `./tests/android/run.sh` from the repository root. The script builds the CLI
and Android x86_64 debug APK, starts a fresh headless KVM emulator, installs the
APK and drives its debug WebView over Chrome DevTools Protocol (Python standard
library only, Python 3.11+). It uses real TLS transfer and native Android commands.

Required user-owned tools:

- Stable Rust, `cargo-tauri` 2 and Android Rust targets (Tauri init installs them).
- `ANDROID_HOME` (default `~/Android/Sdk`) containing platform-tools, emulator,
  command-line tools, Android platform 36, and `system-images;android-34;google_apis;x86_64`.
- `NDK_HOME` (default `$ANDROID_HOME/ndk/28.2.13676358`).
- `JAVA_HOME` (default `~/.local/share/meshdrop-android/jdk-17`). Download a Linux
  x64 Temurin JDK 17 archive from https://adoptium.net/temurin/releases/?version=17
  and extract it there, or set `JAVA_HOME` to an existing compatible JDK.
- Accessible `/dev/kvm`; software CPU emulation is deliberately not substituted.

Install missing SDK components using the SDK's `sdkmanager` in that user-owned
SDK; the script does not install host system packages or a release signing key.
Only Gradle's automatic debug key is used. Runtime AVD data is temporary. Reserve
emulator port 5580 and host ports 19843, 19844 and 19222; run one instance at a time.

The emulator's NAT does not carry host multicast. Pairing uses explicit addresses:
`10.0.2.2:19844` reaches the host receiver; `adb forward tcp:19843 tcp:9843` exposes
the Android receiver to the host CLI. Android app interaction uses debug WebView
CDP; bytes are checked separately using SHA-256. The emulator-only `share-fixture` APK provides read-granted content URIs from a
separate application. `am start -a android.intent.action.SEND` and
`SEND_MULTIPLE` enter its sender activity with a content URI; it constructs the
standard Parcelable stream extras and forwards to meshdrop through its share
filter. This adapter is needed because `am` cannot create `ArrayList<Uri>` extras.
The first share cold-starts meshdrop; the multiple share exercises `onNewIntent`.
Both imports transfer to Linux and are checked byte-for-byte. Actual gallery/SAF
chooser presentation remains a physical acceptance item.

Scenarios cover foreground-service existence, explicitly accepted pairing,
Linux-to-Android acceptance/hash and public MediaStore Download publication,
collision preservation, received-file sharing, Android-to-Linux shared-intent
hashes, rejection without stored content, Home/background reception without automatic
acceptance, and stopping reception. The runner prints logcat (or the emulator log
if ADB is unavailable) on failure, exits nonzero and terminates its child processes.
Notification appearance, real Wi-Fi mDNS, camera optics, Doze and manufacturer
background restrictions require physical-device acceptance.

## Renderer diagnosis and container fallback

Both runners default to `swangle` with Emulator 37.1.11. On this Fedora host,
legacy `swiftshader_indirect` (also selected internally by some guest/off modes)
crashed before boot. GDB/coredump evidence shows `SEGV_ACCERR` at generated code,
with callers in `gles_swiftshader/libGLESv2.so`: a SwiftShader GLES JIT execution
permission fault. Swangle avoids that path and boots successfully. The exact
upstream allocator/permission defect is not established; KVM is usable, and
neither disabling audio/Vulkan nor offscreen Qt fixed the legacy renderer.

Verified: the host runner and the pinned-container runner each passed all 9
scenarios with Emulator 37.1.11 and swangle; the legacy renderer's GDB/coredump
showed SEGV_ACCERR in SwiftShader libGLESv2 generated code.
The successful run also exposed and verified a fix for an already-granted
notification permission leaving the native permission request unresolved.
No additional webview notification capability is granted.

For a stock Debian container fallback, first build the debug APK and `share-fixture` APK using the host
runner prerequisites, then run:

```sh
./tests/android/container.sh
```

The fallback uses Podman, host networking and `/dev/kvm`; mounts only the SDK
read-only, copies filtered project sources and the debug APK, and creates a fresh
AVD inside the container. It does not copy the user's home, credentials or AVDs.
No host system packages are installed. The user's `ge_test` AVD and app minSdk 24 /
targetSdk 36 remain unchanged. The host and container runners serialize access to
their fixed ports using a lock.

The image verifies Google's official Emulator 37.1.11 archive:
`https://dl.google.com/android/repository/emulator-linux_x64-15917651.zip`, SHA-256
`95771e0ae431897b2a4bd2d97fa095f29a8b0624a7b216baf529f9306161c266`.
The checksum is published in the [official emulator archive manifest](https://developer.android.com/studio/emulator_archive).
Set `MESHDROP_EMULATOR_ARCHIVE=/path/to/emulator.zip` to reuse a downloaded copy;
the same checksum is required. `CONTAINER_ENGINE` may select a compatible engine.
`MESHDROP_EMULATOR_GPU` overrides swangle for diagnosis only.

Optional container tracing: `MESHDROP_EMULATOR_TRACE=1 ./tests/android/container.sh`
runs strace for mmap/mprotect/munmap and prints relevant executable mappings and
faults on exit. On both runners this flag also enables emulator `-verbose` and
`-debug-init`; these diagnosis flags are disabled by default. Failure output includes logcat,
Android exit information and the emulator log when ADB is unavailable.

The shared UI scenario shares completed receives from Recent and stops reception
from the single-level Settings list. Unresolved addresses stay in backend/CLI diagnostics. Both runners first
run six activity/focus parser tests. Runtime checks wait for initial boot broadcasts and a stable GMS provider PID
for 30 seconds before launching meshdrop (a fresh-image provider restart can kill
its clients with Android DEPENDENCY DIED), then require the share chooser to be both resumed and focused before
sending Back. They require MainActivity to regain focus with the same process ID
and the original CDP connection; crashes are not hidden by relaunching the app.

The sender fixture is test-only and is installed only on the fresh emulator. It
has no broad storage permission and is never installed on the physical phone.
Build it independently with:

```sh
crates/meshdrop-app/gen/android/gradlew -p tests/android/share-fixture assembleDebug
```

API 29+ public output is checked through `/sdcard/Download`, with saved-location
text in the shared frontend and a second same-name receive preserving both hashes.
JVM tests cover flat names, path traversal, control characters and collision suffixes.
