#!/usr/bin/env bash
set -euo pipefail
apk="$1"
build_tools="${ANDROID_HOME:?}/build-tools/36.0.0"
badging="$("$build_tools/aapt" dump badging "$apk")"
version="$(python3 scripts/packaging/release.py version)"
grep -F "package: name='org.meshdrop.desktop'" <<< "$badging"
grep -F "versionName='$version'" <<< "$badging"
grep -Fx "native-code: 'arm64-v8a'" <<< "$badging"
grep -F "application-label:'meshdrop'" <<< "$badging"
if grep -q 'application-debuggable' <<< "$badging"; then
  echo 'Release APK must not be debuggable' >&2
  exit 1
fi
"$build_tools/apksigner" verify --verbose "$apk"
# Android launch acceptance needs an ARM64 emulator/device; x86 runners cannot
# execute this APK. Metadata/signature validation above is mandatory everywhere.
