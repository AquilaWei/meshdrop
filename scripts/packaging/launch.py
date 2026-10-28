#!/usr/bin/env python3
"""Check a packaged desktop executable survives startup in an isolated profile."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time


def main():
    executable = Path(sys.argv[1]).resolve()
    with tempfile.TemporaryDirectory(prefix='meshdrop-launch-') as profile:
        environment = dict(os.environ, HOME=profile, XDG_CONFIG_HOME=profile,
                           XDG_DATA_HOME=profile, XDG_CACHE_HOME=profile)
        with subprocess.Popen([str(executable)], env=environment) as process:
            try:
                # A successful early exit is also a failed launch check.
                time.sleep(15)
                if process.poll() is not None:
                    raise RuntimeError(f'App exited during startup: {process.returncode}')
                if sys.platform.startswith('linux'):
                    windows = subprocess.check_output(['xwininfo', '-root', '-tree'], text=True)
                    if '"meshdrop"' not in windows:
                        raise RuntimeError('App survived but did not create its desktop window')
                print('Packaged app survived 15 seconds of startup')
            finally:
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()


if __name__ == '__main__':
    main()
