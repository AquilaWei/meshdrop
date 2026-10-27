#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
engine=${CONTAINER_ENGINE:-podman}
"$engine" build -t meshdrop-ui -f tests/ui/Containerfile .
container=$("$engine" create -v meshdrop-ui-registry:/usr/local/cargo/registry -v meshdrop-ui-target:/workspace/target meshdrop-ui)
trap '"$engine" rm -f "$container" >/dev/null' EXIT
tar --exclude=build --exclude=.gradle --exclude=.kotlin --exclude=.tauri --exclude=jniLibs --exclude=local.properties --exclude='*.keystore' --exclude='*.jks' --exclude=.git --exclude=target --exclude=.harnessboard --exclude=.aws --exclude=.codex --exclude=.agents --exclude=.claude --exclude=notes --exclude=CLAUDE.md -cf - . | "$engine" cp - "$container:/workspace/"
"$engine" start -a "$container"
status=$("$engine" inspect --format '{{.State.ExitCode}}' "$container")
if [[ "$status" == 0 ]]; then
  "$engine" cp "$container:/tmp/meshdrop-017-screens" /tmp/
fi
exit "$status"
