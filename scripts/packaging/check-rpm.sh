#!/usr/bin/env bash
# Validate extraction and launch the actual RPM executable on a private display.
set -euo pipefail
rpm_file=$(realpath "$1")
root=$(mktemp -d)
xvfb_pid=
cleanup() {
    result=$?
    if (( result != 0 )); then
        echo "RPM launch failed (exit $result); Xvfb diagnostics:"
        cat "$root/xvfb.log" 2>/dev/null || true
        if [[ -f "$root/extracted/usr/bin/meshdrop-app" ]]; then
            echo 'Executable runtime dependencies (look for not found):'
            ldd "$root/extracted/usr/bin/meshdrop-app" || true
        fi
    fi
    if [[ -n "$xvfb_pid" ]]; then
        kill "$xvfb_pid" 2>/dev/null || true
        wait "$xvfb_pid" 2>/dev/null || true
    fi
    rm -rf "$root"
    exit "$result"
}
trap cleanup EXIT
mkdir "$root/extracted"
# Ubuntu 24.04 rpm2cpio 4.18 returns 1 even for this valid Tauri payload.
# libarchive validates and extracts the RPM directly; no pipeline errors ignored.
bsdtar -xf "$rpm_file" -C "$root/extracted"
test -x "$root/extracted/usr/bin/meshdrop-app"
Xvfb -displayfd 3 -screen 0 1280x1024x24 3>"$root/display" >"$root/xvfb.log" 2>&1 &
xvfb_pid=$!
for attempt in {1..100}; do
    kill -0 "$xvfb_pid"
    if [[ -s "$root/display" ]]; then
        export DISPLAY=":$(cat "$root/display")"
        if xwininfo -root >/dev/null 2>&1; then break; fi
    fi
    sleep 0.1
done
xwininfo -root >/dev/null
timeout 60s dbus-run-session -- python3 scripts/packaging/launch.py "$root/extracted/usr/bin/meshdrop-app"
