"""Real debug WebView/CDP against a CLI peer; no mock transfer implementation."""
import base64
import hashlib
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time
import urllib.request

SDK = Path(os.environ['ANDROID_HOME'])
ADB = str(SDK / 'platform-tools/adb')
SERIAL = 'emulator-5580'
PACKAGE = 'org.meshdrop.desktop'
processes = []

class EmulatorExited(RuntimeError):
    pass

def adb(*args, **kw):
    return subprocess.run([ADB, '-s', SERIAL, *args], check=True, capture_output=True, timeout=20, **kw).stdout

def wait(fn, seconds=60):
    deadline = time.monotonic() + seconds
    last = None
    while time.monotonic() < deadline:
        try:
            value = fn()
            if value:
                return value
        except (Exception,) as error:
            if isinstance(error, EmulatorExited): raise
            last = error
        time.sleep(.2)
    raise AssertionError(f'timed out; last error: {last}')

class CDP:
    def __init__(self, url):
        from urllib.parse import urlparse
        parsed = urlparse(url)
        self.socket = socket.create_connection((parsed.hostname, parsed.port), timeout=30)
        key = base64.b64encode(os.urandom(16)).decode()
        self.socket.sendall(f'GET {parsed.path} HTTP/1.1\r\nHost: {parsed.netloc}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n'.encode())
        header = b''
        while not header.endswith(b'\r\n\r\n'):
            header += self.socket.recv(1)
        assert b' 101 ' in header, header
        self.sequence = 0
    def read(self, count):
        data = b''
        while len(data) < count:
            chunk = self.socket.recv(count-len(data))
            if not chunk:
                raise EOFError('CDP disconnected')
            data += chunk
        return data
    def evaluate(self, expression):
        self.sequence += 1
        body = json.dumps({'id':self.sequence, 'method':'Runtime.evaluate', 'params':{'expression':expression, 'returnByValue':True, 'awaitPromise':True}}).encode()
        mask = os.urandom(4)
        length = len(body)
        header = bytes([129, 128 | length]) if length < 126 else bytes([129, 254])+struct.pack('!H', length)
        self.socket.sendall(header+mask+bytes(value ^ mask[index % 4] for index, value in enumerate(body)))
        while True:
            first, second = self.read(2)
            length = second & 127
            if length == 126: length = struct.unpack('!H', self.read(2))[0]
            if length == 127: length = struct.unpack('!Q', self.read(8))[0]
            data = self.read(length)
            if first & 15 == 8: raise EOFError('CDP closed')
            message = json.loads(data)
            if message.get('id') == self.sequence:
                assert 'error' not in message, message
                result = message['result']
                assert 'exceptionDetails' not in result, result
                return result['result'].get('value')
    def call(self, operation, payload=None):
        return self.evaluate('window.__TAURI__.core.invoke("desktop_request",'+json.dumps({'operation':operation,'payload':payload})+')')
    def click(self, selector):
        self.evaluate("poll()")
        wait(lambda: self.evaluate(f"Boolean(document.querySelector({json.dumps(selector)}))"))
        return self.evaluate(f'document.querySelector({json.dumps(selector)}).click(); true')
    def text(self, selector):
        return self.evaluate(f'document.querySelector({json.dumps(selector)}).textContent')

with tempfile.TemporaryDirectory(prefix='meshdrop-android-') as temporary:
    root = Path(temporary)
    (root/'avd').mkdir()
    (root/'android-user').mkdir()
    env = dict(os.environ, ANDROID_AVD_HOME=str(root/'avd'), ANDROID_USER_HOME=str(root/'android-user'))
    try:
        avdmanager = SDK/'cmdline-tools/latest/bin/avdmanager'
        subprocess.run([str(avdmanager), 'create', 'avd', '-n', 'meshdrop-test', '-k', os.environ.get('MESHDROP_ANDROID_IMAGE','system-images;android-34;google_apis;x86_64'), '--force', '--path', str(root/'avd/meshdrop-test.avd')], input=b'no\n', env=env, check=True, stdout=subprocess.PIPE, timeout=180)
        emulator_log = (root/'emulator.log').open('wb')
        emulator = subprocess.Popen([os.environ.get('MESHDROP_EMULATOR', str(SDK/'emulator/emulator')), '-avd', 'meshdrop-test', '-port', '5580', '-no-window', '-no-audio', '-no-boot-anim', '-no-snapshot', '-gpu', os.environ.get('MESHDROP_EMULATOR_GPU', 'swangle'), '-feature', '-Vulkan', '-show-kernel', *(['-verbose', '-debug-init'] if os.environ.get('MESHDROP_EMULATOR_TRACE') == '1' else []), '-memory', '2048', '-cores', '2', '-accel', 'on'], env=env, stdout=emulator_log, stderr=subprocess.STDOUT)
        processes.append(emulator)
        def booted():
            if emulator.poll() is not None:
                raise EmulatorExited(f'Emulator exited before boot: {emulator.returncode}')
            return adb('shell','getprop','sys.boot_completed').strip() == b'1'
        wait(booted, 600)
        adb('shell','input','keyevent','82')
        apk = Path(os.environ.get('MESHDROP_ANDROID_APK', 'crates/meshdrop-app/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk'))
        adb('install','-r',str(apk))
        adb('shell','pm','grant',PACKAGE,'android.permission.POST_NOTIFICATIONS')
        adb('forward','tcp:19843','tcp:9843')
        adb('logcat','-c')
        adb('shell','am','start','-n',PACKAGE+'/.MainActivity')
        sockets = wait(lambda: [line.split('@')[-1] for line in adb('shell','cat','/proc/net/unix').decode().splitlines() if 'webview_devtools_remote_' in line])
        adb('forward','tcp:19222','localabstract:'+sockets[0])
        def page():
            items = json.loads(urllib.request.urlopen('http://127.0.0.1:19222/json', timeout=3).read())
            return next((item['webSocketDebuggerUrl'] for item in items if item.get('type') == 'page'), None)
        cdp = CDP(wait(page))
        wait(lambda: cdp.evaluate('Boolean(window.__TAURI__)'))
        wait(lambda: 'Background reception enabled' in cdp.text('#android-permission'))
        service = adb('shell','dumpsys','activity','services',PACKAGE).decode()
        assert 'ReceiverService' in service and 'isForeground=true' in service, service
        print('PASS: Android starts a foreground receiver', flush=True)
        cli = root/'cli'; cli.mkdir()
        received = root/'linux-received'; received.mkdir()
        (cli/'config.toml').write_text(f'name = "Linux peer"\nport = 19844\nproviders = []\nreceive_directory = "{received}"\n')
        cli_log = (root/'cli.log').open('w+')
        receiver = subprocess.Popen(['target/debug/meshdrop','--state-dir',str(cli),'receive','--auto-accept'], stdin=subprocess.PIPE, stdout=cli_log, stderr=cli_log, text=True)
        processes.append(receiver)
        wait(lambda: 'listening' in (root/'cli.log').read_text())
        pairing = subprocess.Popen(['target/debug/meshdrop','--state-dir',str(cli),'pair','127.0.0.1:19843','--advertise','10.0.2.2:19844'], stdin=subprocess.PIPE, stdout=cli_log, stderr=cli_log, text=True)
        processes.append(pairing); pairing.stdin.write('y\n'); pairing.stdin.flush()
        invitation = wait(lambda: cdp.call('snapshot')['invitations'])[0]
        code = invitation['detail']['code']
        assert len(code) == 6
        wait(lambda: f'Compare code {code} on BOTH devices' in (root/'cli.log').read_text())
        cdp.click('#invitations [data-action=accept]')
        assert pairing.wait(timeout=30) == 0
        print('PASS: Linux and Android explicitly pair through NAT', flush=True)
        source = root/'linux-to-android.txt'; source.write_bytes(b'Android verified payload\n'*4096)
        send_log = (root/'send.log').open('w+')
        sender = subprocess.Popen(['target/debug/meshdrop','--state-dir',str(cli),'send','127.0.0.1:19843',str(source)], stdout=send_log, stderr=send_log)
        processes.append(sender)
        wait(lambda: cdp.call('snapshot')['invitations'])
        cdp.click('#invitations [data-action=accept]')
        assert sender.wait(timeout=30) == 0
        content = adb('exec-out','run-as',PACKAGE,'cat','files/meshdrop/received/'+source.name)
        assert hashlib.sha256(content).digest() == hashlib.sha256(source.read_bytes()).digest()
        print('PASS: accepted Linux-to-Android content matches SHA-256', flush=True)
        cdp.click('#show-more')
        cdp.click('#received-refresh')
        wait(lambda: source.name in cdp.text('#received-files'))
        cdp.click('#received-files button')
        wait(lambda: 'ChooserActivity' in adb('shell','dumpsys','activity','activities').decode() or 'android.intent.action.CHOOSER' in adb('shell','dumpsys','activity','activities').decode())
        adb('shell','input','keyevent','4')
        print('PASS: received content can be shared without adb access', flush=True)
        cdp.call('add_device', {'address':'10.0.2.2:19844'}); cdp.call('refresh'); cdp.evaluate('poll()')
        wait(lambda: 'Paired' in cdp.text('#devices'))
        cdp.call('add_device', {'address':'127.0.0.1:1'}); cdp.call('refresh'); cdp.evaluate('poll()')
        wait(lambda: '127.0.0.1:1' in cdp.text('#problems'))
        assert '127.0.0.1:1' not in cdp.text('#devices')
        print('PASS: unresolved candidate stays in More diagnostics', flush=True)
        cdp.call('select_files', ['/data/user/0/'+PACKAGE+'/files/meshdrop/received/'+source.name])
        cdp.click('#show-send')
        cdp.click('#devices input[type=checkbox]'); cdp.click('#send')
        wait(lambda: (received/source.name).exists())
        assert hashlib.sha256((received/source.name).read_bytes()).digest() == hashlib.sha256(source.read_bytes()).digest()
        print('PASS: Android-to-Linux content matches SHA-256', flush=True)
        rejected = root/'rejected.txt'; rejected.write_bytes(b'must not arrive')
        sender = subprocess.Popen(['target/debug/meshdrop','--state-dir',str(cli),'send','127.0.0.1:19843',str(rejected)], stdout=send_log, stderr=send_log)
        processes.append(sender)
        wait(lambda: cdp.call('snapshot')['invitations']); cdp.click('#invitations [data-action=reject]')
        assert sender.wait(timeout=30) != 0
        assert adb('shell','run-as',PACKAGE,'ls','-A','files/meshdrop/received').decode().strip() == source.name
        print('PASS: rejected invitation writes no content', flush=True)
        adb('shell','input','keyevent','3')
        sender = subprocess.Popen(['target/debug/meshdrop','--state-dir',str(cli),'send','127.0.0.1:19843',str(rejected)], stdout=send_log, stderr=send_log)
        processes.append(sender)
        wait(lambda: cdp.call('snapshot')['invitations'])
        assert 'isForeground=true' in adb('shell','dumpsys','activity','services',PACKAGE).decode()
        adb('shell','am','start','-n',PACKAGE+'/.MainActivity'); cdp.click('#invitations [data-action=reject]')
        assert sender.wait(timeout=30) != 0
        print('PASS: background reception retains explicit user decision', flush=True)
        cdp.click('#show-more')
        cdp.click('#stop-receiver')
        wait(lambda: 'isForeground=true' not in adb('shell','dumpsys','activity','services',PACKAGE).decode())
        print('PASS: stop receiving action stops foreground service', flush=True)
        print('9 Android scenarios passed; 0 failed', flush=True)
    except Exception:
        try:
            print(adb('logcat','-b','crash','-d').decode(errors='replace'), flush=True)
            print(adb('shell','dumpsys','activity','exit-info',PACKAGE).decode(errors='replace'), flush=True)
            print(adb('logcat','-d','-t','300').decode(errors='replace'), flush=True)
        except Exception as diagnostic:
            print('logcat unavailable:', diagnostic, flush=True)
            print((root/'emulator.log').read_text(errors='replace')[-12000:], flush=True)
        raise
    finally:
        try:
            subprocess.run([ADB,'-s',SERIAL,'emu','kill'], capture_output=True, timeout=10)
        except subprocess.TimeoutExpired:
            print('ADB emulator shutdown timed out; terminating owned process', flush=True)
        for process in reversed(processes):
            if process.poll() is None:
                process.terminate()
                try: process.wait(timeout=10)
                except subprocess.TimeoutExpired: process.kill(); process.wait()
