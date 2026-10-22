#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
engine=${CONTAINER_ENGINE:-podman}
"$engine" build -t meshdrop-wine -f tests/wine/Containerfile .
# The container receives a copy; no Wine or cross toolchain is installed on host.
container=$("$engine" create meshdrop-wine)
trap '"$engine" rm -f "$container" >/dev/null' EXIT
tar --exclude=.git --exclude=target --exclude=.harnessboard --exclude=.aws --exclude=.codex --exclude=.agents --exclude=.claude --exclude=notes --exclude=CLAUDE.md -cf - . | "$engine" cp - "$container:/workspace/"
"$engine" start -a "$container"
status=$("$engine" inspect --format '{{.State.ExitCode}}' "$container")
exit "$status"
