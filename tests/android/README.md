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
CDP; bytes are checked separately using SHA-256. Test source selection supplies a
path through the existing command, as the desktop WebDriver suite does; actual
SAF chooser interaction remains a native acceptance item.

Scenarios cover foreground-service existence, explicitly accepted pairing,
Linux-to-Android acceptance/hash, a FileProvider share chooser, Android-to-Linux
hash, rejection without stored content, Home/background reception without automatic
acceptance, and stopping reception. The runner prints logcat (or the emulator log
if ADB is unavailable) on failure, exits nonzero and terminates its child processes.
Notification appearance, real Wi-Fi mDNS, camera optics, Doze and manufacturer
background restrictions require physical-device acceptance.

## Host execution evidence and limits

Android runtime verification is blocked on this Fedora host before the app can be
installed. KVM's `emulator -accel-check` reports usable KVM (exit 0), but Google
Emulator 37.1.11 and an isolated 36.3.10 both terminate with SIGSEGV during guest
startup. This is a native emulator failure, not a passing app test. The underlying
library/driver cause is unresolved: `coredumpctl info 2871558` (exit 0) confirms
signal 11 and only an unresolved instruction address in the stack. It does not
establish a specific renderer defect.

The final bounded API34 attempts used Emulator 36.3.10 with `swiftshader_indirect`
and `off`; each runner exited 1, reporting emulator return code -11 before boot.
The earlier API35 `-gpu off` attempt likewise exited 1 / emulator -11. Logs:

- `/tmp/meshdrop-stage3-coredump.log`
- `/tmp/meshdrop-stage3-emulator-off.log`
- `/tmp/meshdrop-stage3-api34-swiftshader.log`
- `/tmp/meshdrop-stage3-api34-off.log`

No Android E2E scenario, camera permission/scan, notification appearance, native
SAF chooser or share intent has been demonstrated running successfully here.
Debug APK compilation and JVM safety/lifecycle tests are separate successful
checks. The script is retained for execution from a normal user terminal on a
working emulator environment; no more emulator retries were made after those
two API34 modes. The user's `ge_test` AVD was not modified.

To override tools or the test image without replacing the user's SDK:

```sh
MESHDROP_EMULATOR="$HOME/.local/share/meshdrop-android/emulator-36/emulator/emulator" \
MESHDROP_EMULATOR_GPU=off \
MESHDROP_ANDROID_IMAGE='system-images;android-34;google_apis;x86_64' \
./tests/android/run.sh
```

The emulator archive is available at
https://developer.android.com/studio/emulator_archive. No host packages are
installed and the app's minSdk 24 / targetSdk 36 are unchanged.
