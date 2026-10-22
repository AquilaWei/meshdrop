"""Real Tauri WebDriver and a separate meshdrop CLI process."""
import hashlib
import json
import os
from pathlib import Path
import re
import socket
import subprocess
import tempfile
import time
import urllib.request

ELEMENT = "element-6066-11e4-a52e-4f735466cecf"
BASE = "http://127.0.0.1:4444"
session = None
processes = []


def http(method, path, body=None):
    data = None if body is None else json.dumps(body).encode()
    request = urllib.request.Request(BASE + path, data=data, method=method, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(request, timeout=30) as response:
        value = json.load(response)["value"]
    if isinstance(value, dict) and "error" in value:
        raise RuntimeError(value)
    return value


def wait(predicate, timeout=30):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            value = predicate()
            if value:
                return value
        except Exception as error:
            last = error
        time.sleep(0.15)
    raise AssertionError(f"condition timed out; last error: {last}")


def js(script, args=None):
    return http("POST", f"/session/{session}/execute/sync", {"script": script, "args": args or []})


def click(selector):
    node = http("POST", f"/session/{session}/element", {"using": "css selector", "value": selector})
    http("POST", f"/session/{session}/element/{node[ELEMENT]}/click", {})


def fill(selector, value):
    node = http("POST", f"/session/{session}/element", {"using": "css selector", "value": selector})
    http("POST", f"/session/{session}/element/{node[ELEMENT]}/clear", {})
    http("POST", f"/session/{session}/element/{node[ELEMENT]}/value", {"text": value})


def text(selector):
    return js("return document.querySelector(arguments[0]).textContent", [selector])


def launch(args, **kwargs):
    process = subprocess.Popen(args, **kwargs)
    processes.append(process)
    return process


def port_open(port):
    try:
        with socket.create_connection(("127.0.0.1", port), timeout=0.3):
            return True
    except OSError:
        return False


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def config(directory, name, port):
    directory.mkdir()
    received = directory / "received"
    (directory / "config.toml").write_text(f'name = {json.dumps(name)}\nport = {port}\nreceive_directory = {json.dumps(str(received))}\nproviders = ["mdns", "manual", "peer-list"]\ninvitation_timeout_seconds = 15\nidle_timeout_seconds = 5\n')
    return received


def main(root):
    global session
    cli_dir, app_dir = root / "cli", root / "app"
    peer_name = "CLI <img src=x onerror=alert(1)>"
    cli_received = config(cli_dir, peer_name, 19844)
    app_received = config(app_dir, "Desktop under test", 19843)
    cli = str(Path("target/debug/meshdrop").resolve())
    cli_log = open(root / "cli.log", "w+")
    launch([cli, "--state-dir", str(cli_dir), "receive", "--auto-accept"], stdin=subprocess.PIPE, stdout=cli_log, stderr=cli_log)
    wait(lambda: port_open(19844))
    driver_log = open(root / "driver.log", "w+")
    env = dict(os.environ, MESHDROP_STATE_DIR=str(app_dir))
    launch(["tauri-driver"], env=env, stdout=driver_log, stderr=driver_log)
    wait(lambda: http("GET", "/status"))
    session = http("POST", "/session", {"capabilities": {"alwaysMatch": {"tauri:options": {"application": str(Path("target/debug/meshdrop-app").resolve())}}}})["sessionId"]
    wait(lambda: "Receiving on port 19843" in text("#service-state"))
    wait(lambda: peer_name in text("#devices"), 45)
    assert js("return document.querySelector('#qr').width > 100")
    assert "meshdrop://" in text("#qr-content")
    print("PASS: app background receiver, mDNS device list and own QR", flush=True)

    pair_log = open(root / "pair.log", "w+")
    pairing = launch([cli, "--state-dir", str(cli_dir), "pair", "127.0.0.1:19843", "--advertise", "127.0.0.1:19844"], stdin=subprocess.PIPE, stdout=pair_log, stderr=pair_log)
    pairing.stdin.write(b"y\n"); pairing.stdin.flush()
    wait(lambda: bool(js("return document.querySelector('#invitations [data-action=accept]')")))
    code = text("#invitations .code")
    assert re.fullmatch(r"\d{6}", code)
    pair_log.flush(); assert code in (root / "pair.log").read_text()
    click("#invitations [data-action=accept]")
    assert pairing.wait(timeout=20) == 0
    click("#refresh")
    wait(lambda: "Paired" in text("#devices"))
    print("PASS: CLI pairing requires matching code and in-app acceptance", flush=True)

    # WebDriver controls the webview, not native GTK chooser windows. Inject only
    # the chooser's result through the same existing Rust command used by the
    # dialog callback. Transport, state, UI rendering and filesystem remain real.
    source = root / "payload <img src=x onerror=alert(1)>.txt"
    source.write_bytes(b"meshdrop ui payload\n" * 131072)
    click("#devices input[type=checkbox]:not(:disabled)")
    selection = http("POST", f"/session/{session}/execute/async", {
        "script": "const done = arguments[arguments.length - 1]; window.__TAURI__.core.invoke('desktop_request', {operation:'select_files', payload:arguments[0]}).then(result => done({result})).catch(error => done({error:String(error)}));",
        "args": [[str(source)]],
    })
    assert "error" not in selection, selection
    wait(lambda: source.name in text("#selected-files"))
    click("#send")
    wait(lambda: "complete" in text("#transfers"))
    wait(lambda: (cli_received / "payload _img src=x onerror=alert(1)_.txt").exists())
    assert digest(source) == digest(cli_received / "payload _img src=x onerror=alert(1)_.txt")
    assert js("return [...document.querySelectorAll('#transfers progress')].some(p => p.value === 100)")
    print("PASS: injected chooser result, app-to-CLI send, 100% progress and SHA-256", flush=True)

    incoming = root / "from-cli.txt"; incoming.write_bytes(b"CLI to desktop\n" * 8192)
    send_log = open(root / "send.log", "w+")
    sending = launch([cli, "--state-dir", str(cli_dir), "send", "127.0.0.1:19843", str(incoming)], stdout=send_log, stderr=send_log)
    wait(lambda: incoming.name in text("#invitations"))
    click("#invitations [data-action=accept]")
    assert sending.wait(timeout=20) == 0
    wait(lambda: (app_received / incoming.name).exists())
    assert digest(incoming) == digest(app_received / incoming.name)
    wait(lambda: js("return [...document.querySelectorAll('.transfer-status')].filter(n => n.textContent === 'complete').length >= 2"))
    print("PASS: CLI-to-app invitation, in-app acceptance, completion and SHA-256", flush=True)

    rejected = root / "rejected.txt"; rejected.write_text("must not arrive")
    rejecting = launch([cli, "--state-dir", str(cli_dir), "send", "127.0.0.1:19843", str(rejected)], stdout=send_log, stderr=send_log)
    wait(lambda: rejected.name in text("#invitations"))
    click("#invitations [data-action=reject]")
    assert rejecting.wait(timeout=20) != 0
    assert not (app_received / rejected.name).exists()
    assert not list(app_received.glob("*.part"))
    print("PASS: in-app rejection leaves no content or temporary files", flush=True)

    assert peer_name in text("#devices")
    assert not js("return document.querySelector('#devices img, #invitations img, #transfers img')")
    assert source.name in text("#transfers")
    print("PASS: malicious device names and filenames display as text", flush=True)

    js("document.querySelector('#settings-toml').closest('details').open = true; return true")
    fill("#settings-toml", "prot = 1234")
    click("#save-toml")
    wait(lambda: "config.toml" in text("#error") and "prot" in text("#error"))
    assert "port = 19843" in (app_dir / "config.toml").read_text()
    print("PASS: invalid TOML reports path and preserves saved settings", flush=True)
    print("7 UI scenarios passed; 0 failed", flush=True)


with tempfile.TemporaryDirectory(prefix="meshdrop-ui-") as folder:
    root = Path(folder)
    try:
        main(root)
    except Exception:
        if session:
            try:
                print(js("return document.body.innerText"), flush=True)
            except Exception:
                pass
        for log in root.glob("*.log"):
            print(f"--- {log.name} ---\n{log.read_text()[-16000:]}", flush=True)
        raise
    finally:
        if session:
            try:
                http("DELETE", f"/session/{session}")
            except Exception:
                pass
        for process in reversed(processes):
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill(); process.wait()
