#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
engine=${CONTAINER_ENGINE:-podman}
sdk=${ANDROID_HOME:-$HOME/Android/Sdk}
apk=crates/meshdrop-app/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
[[ -e /dev/kvm && -f "$apk" && -x "$sdk/cmdline-tools/latest/bin/avdmanager" ]] || { echo 'Requires /dev/kvm, an existing x86_64 debug APK and Android SDK; see tests/android/README.md.' >&2; exit 1; }
exec 9>"/tmp/meshdrop-android-${UID}.lock"
flock -n 9 || { echo 'Another meshdrop Android runner is active.' >&2; exit 1; }
context=$(mktemp -d /tmp/meshdrop-android-container.XXXXXX)
container=
cleanup() { if [[ -n "$container" ]]; then "$engine" rm -f "$container" >/dev/null; fi; rm -rf "$context"; }
trap cleanup EXIT
cp tests/android/Containerfile "$context/Containerfile"
if [[ -n ${MESHDROP_EMULATOR_ARCHIVE:-} ]]; then
  cp "$MESHDROP_EMULATOR_ARCHIVE" "$context/emulator.zip"
else
  curl --fail --location --retry 3 https://dl.google.com/android/repository/emulator-linux_x64-15917651.zip -o "$context/emulator.zip"
fi
echo "95771e0ae431897b2a4bd2d97fa095f29a8b0624a7b216baf529f9306161c266  $context/emulator.zip" | sha256sum -c -
"$engine" build -t meshdrop-android -f "$context/Containerfile" "$context"
# Host networking keeps the CLI, emulator NAT and forwarded ADB/CDP ports local.
# No home directory, credentials, keystore or user AVD is copied into the image.
container=$("$engine" create --network host --device /dev/kvm --group-add keep-groups --security-opt label=disable -v "$sdk:/opt/android-sdk:ro" -v meshdrop-android-target:/workspace/target -v meshdrop-android-registry:/usr/local/cargo/registry -e MESHDROP_EMULATOR="${MESHDROP_CONTAINER_EMULATOR:-/opt/emulator/emulator}" -e MESHDROP_EMULATOR_TRACE="${MESHDROP_EMULATOR_TRACE:-0}" -e MESHDROP_EMULATOR_GPU="${MESHDROP_EMULATOR_GPU:-swangle}" meshdrop-android)
git ls-files --cached --others --exclude-standard -z | tar --null --exclude=build --exclude=.gradle --exclude=.kotlin --exclude=.tauri --exclude=jniLibs --exclude=local.properties --exclude='*.keystore' --exclude='*.jks' --exclude=.git --exclude=target --exclude=.harnessboard --exclude=.aws --exclude=.codex --exclude=.agents --exclude=.claude --exclude=notes --exclude=CLAUDE.md -T - -cf - | "$engine" cp - "$container:/workspace/"
"$engine" cp "$apk" "$container:/workspace/app.apk"
"$engine" cp tests/android/share-fixture/build/outputs/apk/debug/meshdrop-share-fixture-debug.apk "$container:/workspace/share-fixture.apk"
"$engine" start -a "$container"
status=$("$engine" inspect --format '{{.State.ExitCode}}' "$container")
exit "$status"
