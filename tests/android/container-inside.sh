#!/usr/bin/env bash
set -euo pipefail
# SDK is read-only; AVDs, signing state and emulator data stay in this container.
export JAVA_HOME=$(dirname "$(dirname "$(readlink -f "$(command -v java)")")")
if [[ ${MESHDROP_EMULATOR_TRACE:-0} == 1 ]]; then
  export MESHDROP_TRACED_EMULATOR=$MESHDROP_EMULATOR
  cat > /tmp/emulator-trace <<'TRACE'
#!/bin/sh
exec strace -f -e trace=mmap,mprotect,munmap -o /tmp/emulator-syscalls.log "$MESHDROP_TRACED_EMULATOR" "$@"
TRACE
  chmod +x /tmp/emulator-trace
  export MESHDROP_EMULATOR=/tmp/emulator-trace
  trap 'grep -E "PROT_EXEC|SIGSEGV|EPERM|EACCES|EINVAL" /tmp/emulator-syscalls.log | tail -300' EXIT
fi
cargo build -p meshdrop-cli --locked && python3 tests/android/scenario.py
