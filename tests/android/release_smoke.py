"""Launch the release arm64 APK on a private AVD with native ARM translation."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

from activity import foreground_activity

SDK = Path(os.environ['ANDROID_HOME'])
ADB = str(SDK / 'platform-tools/adb')
PACKAGE = 'org.meshdrop.desktop'
SERIAL = 'emulator-5580'


def adb(*arguments, timeout=30):
    return subprocess.check_output([ADB, '-s', SERIAL, *arguments], timeout=timeout,
                                   stderr=subprocess.PIPE)


def process_pid(package):
    """An absent process is empty; transport errors must still fail the test."""
    try:
        return adb('shell', 'pidof', package).strip()
    except subprocess.CalledProcessError as error:
        if error.returncode != 1 or error.stderr:
            raise
        return b''


def app_pid():
    """Keep a disappeared app distinct from a failed ADB connection."""
    return process_pid(PACKAGE)


def provider_pid():
    """A provider absent during first boot is transient; ADB failures still fail."""
    return process_pid('com.google.android.gms.persistent')


def failure_diagnostics(pid):
    """Preserve kill reasons as well as crashes before destroying the private AVD."""
    commands = [
        ('crash buffer', ['logcat', '-b', 'crash', '-d']),
        ('activity and process lifecycle', ['logcat', '-b', 'all', '-d', '-v',
                                          'threadtime']),
        ('last app exit', ['shell', 'dumpsys', 'activity', 'exit-info', PACKAGE]),
    ]
    for label, arguments in commands:
        result = subprocess.run([ADB, '-s', SERIAL, *arguments], capture_output=True,
                                text=True, timeout=30)
        output = result.stdout
        if label == 'activity and process lifecycle':
            tokens = (PACKAGE, 'ActivityManager', 'ActivityTaskManager',
                      'AndroidRuntime', 'lmkd', 'lowmemorykiller')
            output = '\n'.join(line for line in output.splitlines()
                               if any(token in line for token in tokens)
                               or (pid and pid.decode() in line.split()))
        print(f'=== {label} (exit {result.returncode}) ===\n{output}\n{result.stderr}', flush=True)


def wait(check, seconds=180):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if check():
            return
        time.sleep(1)
    raise RuntimeError('Timed out waiting for Android startup')


def launch(apk):
    with tempfile.TemporaryDirectory(prefix='meshdrop-release-avd-') as temporary:
        root = Path(temporary)
        (root / 'avd').mkdir()
        environment = dict(os.environ, ANDROID_AVD_HOME=str(root / 'avd'),
                           ANDROID_USER_HOME=str(root / 'android-user'))
        subprocess.run([str(SDK / 'cmdline-tools/latest/bin/avdmanager'), 'create',
                        'avd', '-n', 'meshdrop-test', '-k',
                        'system-images;android-34;google_apis;x86_64', '--force',
                        '--path', str(root / 'avd/meshdrop-test.avd')],
                       input=b'no\n', env=environment, check=True, timeout=180)
        with (root / 'emulator.log').open('w+') as log:
            process = subprocess.Popen([
                str(SDK / 'emulator/emulator'), '-avd', 'meshdrop-test', '-port',
                '5580', '-no-window', '-no-audio', '-no-boot-anim', '-no-snapshot',
                '-gpu', 'swangle', '-feature', '-Vulkan', '-memory', '2048',
                '-cores', '2', '-accel', 'on'], env=environment, stdout=log,
                stderr=subprocess.STDOUT)
            pid = b''
            try:
                def booted():
                    if process.poll() is not None:
                        raise RuntimeError(f'Emulator exited: {process.returncode}')
                    try:
                        return adb('shell', 'getprop', 'sys.boot_completed').strip() == b'1'
                    except subprocess.CalledProcessError:
                        return False
                wait(booted, 600)
                print('Guest ABI list:', adb('shell', 'getprop', 'ro.product.cpu.abilist').decode().strip(), flush=True)
                print('Native bridge:', adb('shell', 'getprop', 'ro.dalvik.vm.native.bridge').decode().strip(), flush=True)
                adb('shell', 'input', 'keyevent', '82')
                print(adb('install', str(apk), timeout=180).decode(), flush=True)
                adb('shell', 'pm', 'grant', PACKAGE, 'android.permission.POST_NOTIFICATIONS')
                adb('shell', 'am', 'wait-for-broadcast-idle', timeout=120)
                # Fresh GMS provider restarts can kill dependent apps; settle it first.
                state = {'pid': None, 'since': time.monotonic()}
                def stable():
                    pid = provider_pid()
                    if pid != state['pid']:
                        state.update(pid=pid, since=time.monotonic())
                    return pid and time.monotonic() - state['since'] >= 30
                wait(stable)
                adb('shell', 'input', 'keyevent', '82')
                print(adb('shell', 'am', 'start', '-W', '-n', PACKAGE + '/.MainActivity').decode(), flush=True)
                def focused():
                    return foreground_activity(
                        adb('shell', 'dumpsys', 'activity', 'activities').decode(),
                        adb('shell', 'dumpsys', 'window').decode()) == PACKAGE + '.MainActivity'
                wait(focused)
                pid = app_pid()
                time.sleep(15)
                retained_pid = app_pid()
                if not retained_pid:
                    raise RuntimeError(f'Release app process gone during 15s retention; initial PID {pid.decode()}')
                if not pid or retained_pid != pid or not focused():
                    raise RuntimeError('Release app did not retain its process and focused activity')
                print(f'PASS: release APK installed, launched and retained focused MainActivity for 15s; PID {pid.decode()}', flush=True)
                adb('shell', 'am', 'force-stop', PACKAGE)
            except Exception:
                log.flush()
                print((root / 'emulator.log').read_text()[-8000:], flush=True)
                failure_diagnostics(pid)
                raise
            finally:
                # Terminate only the emulator process created by this test.
                process.terminate()
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


if __name__ == '__main__':
    import fcntl
    with open(f'/tmp/meshdrop-android-{os.getuid()}.lock', 'w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        launch(Path(sys.argv[1]).resolve())
