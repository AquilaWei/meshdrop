"""Real Tauri WebDriver and a separate meshdrop CLI process."""
import hashlib
from http.client import RemoteDisconnected
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
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            value = json.load(response)["value"]
    except RemoteDisconnected:
        # WebKitWebDriver occasionally closes a pooled HTTP connection. Retry
        # read-only observations only; never replay a click, key or command.
        read_only = method == "GET" or (path.endswith('/execute/sync') and body['script'].startswith('return '))
        if not read_only:
            raise
        print(f"WebDriver connection closed during read: {path}; retrying observation", flush=True)
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
    try:
        http("POST", f"/session/{session}/element/{node[ELEMENT]}/value", {"text": value})
    except RemoteDisconnected:
        # The remote may have applied the text before closing its response.
        # Verify the resulting value instead of replaying native key input.
        wait(lambda: js('return document.querySelector(arguments[0]).value', [selector]) == value)
        print(f"WebDriver text response closed; verified completed value for {selector}", flush=True)


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
    receiver = launch([cli, "--state-dir", str(cli_dir), "receive", "--auto-accept"], stdin=subprocess.PIPE, stdout=cli_log, stderr=cli_log)
    wait(lambda: port_open(19844))
    driver_log = open(root / "driver.log", "w+")
    env = dict(os.environ, MESHDROP_STATE_DIR=str(app_dir))
    launch(["tauri-driver"], env=env, stdout=driver_log, stderr=driver_log)
    wait(lambda: http("GET", "/status"))
    session = http("POST", "/session", {"capabilities": {"alwaysMatch": {"tauri:options": {"application": str(Path("target/debug/meshdrop-app").resolve())}}}})["sessionId"]
    wait(lambda: "Receiving on port 19843" in text("#service-state"))
    # Keep the native refresh control focused while a real search runs. Disabling
    # the old button removes it from keyboard navigation on every timer tick.
    wait(lambda: js("return !refreshing"))
    js("document.querySelector('#refresh').focus(); window.refreshNode = document.querySelector('#refresh'); const original = act; window.refreshCalls = 0; act = (operation,payload) => { if (operation === 'refresh') window.refreshCalls++; return original(operation,payload); }; refresh(); refresh(); act = original; return true")
    assert js("return window.refreshCalls") == 1
    assert js("return !window.refreshNode.disabled"), "refresh must remain focusable during background search"
    assert js("return window.refreshNode.getAttribute('aria-busy') === 'true'"), "search must expose its active state"
    assert js("return document.activeElement === window.refreshNode"), "search stole keyboard focus"
    assert js("return getComputedStyle(window.refreshNode.querySelector('svg')).animationName") == "spin"
    wait(lambda: js("return !refreshing"))
    js("window.refreshMutations = 0; window.refreshObserver = new MutationObserver(changes => window.refreshMutations += changes.length); window.refreshObserver.observe(window.refreshNode,{childList:true,subtree:true,attributes:true,attributeFilter:['class','style','disabled']}); return true")
    time.sleep(1.1)  # More than three 300ms snapshots must not restyle the control.
    assert js("return window.refreshMutations") == 0
    assert js("return document.activeElement === window.refreshNode")
    js("window.refreshObserver.disconnect(); return true")
    assert js("return document.querySelector('#refresh') === window.refreshNode"), "poll replaced refresh control"
    assert js("return window.refreshNode.getAttribute('aria-busy') === 'false'")
    print("PASS: refresh starts/stops without disabling, replacing or stealing focus", flush=True)
    assert not js("return document.querySelector('#send, #devices input[type=checkbox]')")
    wait(lambda: peer_name in text("#devices"), 45)
    assert js("return document.querySelector('#qr').width > 100")
    assert "meshdrop://" in text("#qr-content")
    print("PASS: app background receiver, mDNS device list and own QR", flush=True)

    assert js("return document.querySelector('#more-page').hidden")
    click("#show-more")
    fill("#address", "127.0.0.1:1")
    click("#manual button")
    wait(lambda: "Added." in text("#add-status"))
    click("#show-send")
    click("#refresh")
    wait(lambda: js("return !refreshing"))
    js("window.probeState=null; request('snapshot').then(value=>window.probeState=value); return true")
    wait(lambda: js("return window.probeState !== null"))
    assert js("return probeState.problems.some(item=>item.error.includes('127.0.0.1:1'))")
    assert "127.0.0.1:1" not in text("#devices")
    print("PASS: unresolved candidate stays out of the grid; diagnostics remain in the backend", flush=True)

    # WebDriver controls the webview, not native GTK chooser windows. Inject only
    # the chooser's result through the same existing Rust command used by the
    # dialog callback. Transport, state, UI rendering and filesystem remain real.
    source = root / "payload <img src=x onerror=alert(1)>.txt"
    source.write_bytes(b"meshdrop ui payload\n" * 131072)
    second_source = root / "second.txt"
    second_source.write_text("second selected file")
    js("""window.realAct = act;
        window.chooserPaths = [arguments[0]];
        window.chooserCalls = 0; window.sendCalls = 0;
        act = (operation, payload) => {
            if (operation === 'pick_files') {
                window.chooserCalls++;
                return realAct('append_files', chooserPaths);
            }
            if (operation === 'send') window.sendCalls++;
            return realAct(operation, payload);
        }; return true;""", [str(source)])
    click("#devices button:not(:disabled)")
    wait(lambda: js("return document.querySelector('#outgoing-pair').open"))
    assert js("return window.chooserCalls") == 0
    assert js("return window.sendCalls") == 0
    assert js("return chosenFiles.length") == 0
    click("#pair-dismiss")
    print("PASS: unpaired avatar with no files only starts pairing; no picker or invite", flush=True)
    click("#pick-files")
    wait(lambda: source.name in text("#selected-files"))
    js("window.chooserPaths=[arguments[0]]; return true", [str(second_source)])
    click("#pick-files")
    wait(lambda: second_source.name in text("#selected-files"))
    assert js("return chosenFiles.length") == 2
    assert js("return window.chooserCalls") == 2
    print("PASS: plus is the picker entry and separate picks accumulate files", flush=True)
    click("#devices button:not(:disabled)")
    wait(lambda: js("return document.querySelector('#outgoing-pair').open"))
    code = text("#pair-code")
    assert re.fullmatch(r"\d{6}", code)
    click("#pair-confirm")
    wait(lambda: code in (root / "cli.log").read_text())
    receiver.stdin.write(b"y\n"); receiver.stdin.flush()
    wait(lambda: not js("return document.querySelector('#outgoing-pair').open"))
    wait(lambda: "Connected" in text("#devices"))
    assert js("return window.sendCalls") == 0
    assert js("return currentTransfers.length") == 0
    assert js("return window.chooserCalls") == 2
    print("PASS: unpaired avatar with selected files pairs and becomes connected without inviting", flush=True)
    click("#devices button:not(:disabled)")
    wait(lambda: "Complete" in text("#transfers"))
    assert js("return window.sendCalls") == 1
    assert js("return currentTransfers.length") == 1
    assert js("return currentTransfers[0].files.length") == 2
    wait(lambda: (cli_received / second_source.name).exists())
    assert digest(second_source) == digest(cli_received / second_source.name)
    print("PASS: multiple files produce exactly one invite and both arrive with matching hashes", flush=True)
    wait(lambda: (cli_received / "payload _img src=x onerror=alert(1)_.txt").exists())
    assert digest(source) == digest(cli_received / "payload _img src=x onerror=alert(1)_.txt")
    assert js("return [...document.querySelectorAll('#transfers progress')].some(p => p.value === 100)")
    sent_id = js("return currentTransfers.find(item => item.direction === 'send').id")
    known_path = js("window.knownPath = null; act('known_file',{id:arguments[0],name:arguments[1]}).then(value => window.knownPath = value); return true", [sent_id, source.name])
    wait(lambda: js("return window.knownPath") == str(source))
    print("PASS: injected chooser result, app-to-CLI send, recorded reveal path, 100% progress and SHA-256", flush=True)

    click("#selected-files button")
    wait(lambda: js("return chosenFiles.length === 1"))
    assert second_source.name in text("#selected-files")
    click("#selected-files button")
    wait(lambda: js("return chosenFiles.length === 0"))
    print("PASS: each selected file can be removed independently", flush=True)
    click("#devices button:not(:disabled)")
    wait(lambda: "Press ＋" in text("#send-hint"))
    assert js("return document.querySelector('#send-hint').getAttribute('aria-live')") == "polite"
    assert js("return window.sendCalls") == 1
    assert js("return window.chooserCalls") == 2
    assert not js("return document.querySelector('#outgoing-pair').open")
    print("PASS: paired avatar without files announces the plus-first hint; no picker or invite", flush=True)
    js("window.chooserPaths=[arguments[0]]; return true", [str(source)])
    click("#pick-files")
    wait(lambda: js("return chosenFiles.length === 1"))
    click("#devices button:not(:disabled)")
    wait(lambda: js("return document.querySelectorAll('#transfers .transfer').length === 2"))
    assert js("return window.sendCalls") == 2
    assert not js("return document.querySelector('#outgoing-pair').open")
    wait(lambda: js("return [...document.querySelectorAll('.transfer-status')].filter(n => n.textContent === 'Complete').length === 2"))
    print("PASS: paired avatar with files sends without a code prompt or another picker", flush=True)
    js("window.realDevices=[...deviceItems.values()]; devices([{...realDevices[0],mismatch:true}]); return true")
    assert js("return document.querySelector('#devices .device').disabled")
    js("document.querySelector('#devices .device').click(); return true")
    assert js("return window.sendCalls") == 2
    assert js("return window.chooserCalls") == 3
    js("devices(realDevices); return true")
    print("PASS: fingerprint mismatch disables avatar activation", flush=True)

    incoming_target = next(iter(json.loads((cli_dir / "trust.json").read_text())))
    incoming = root / "from-cli.txt"; incoming.write_bytes(b"CLI to desktop\n" * 8192)
    send_log = open(root / "send.log", "w+")
    sending = launch([cli, "--state-dir", str(cli_dir), "send", incoming_target, str(incoming)], stdout=send_log, stderr=send_log)
    wait(lambda: incoming.name in text("#invitations"))
    click("#invitations [data-action=accept]")
    assert sending.wait(timeout=20) == 0
    wait(lambda: (app_received / incoming.name).exists())
    assert digest(incoming) == digest(app_received / incoming.name)
    wait(lambda: js("return [...document.querySelectorAll('.transfer-status')].filter(n => n.textContent === 'Complete').length >= 3"))
    wait(lambda: str(app_received / incoming.name) in text("#transfers"))
    print("PASS: CLI-to-app acceptance, SHA-256 and saved-location display", flush=True)

    rejected = root / "rejected.txt"; rejected.write_text("must not arrive")
    rejecting = launch([cli, "--state-dir", str(cli_dir), "send", incoming_target, str(rejected)], stdout=send_log, stderr=send_log)
    wait(lambda: rejected.name in text("#invitations"))
    http("POST", f"/session/{session}/actions", {"actions":[{"type":"key","id":"keyboard","actions":[{"type":"keyDown","value":"\ue00c"},{"type":"keyUp","value":"\ue00c"}]}]})
    assert rejecting.wait(timeout=20) != 0
    assert not (app_received / rejected.name).exists()
    assert not list(app_received.glob("*.part"))
    print("PASS: in-app rejection leaves no content or temporary files", flush=True)

    assert peer_name in text("#devices")
    assert not js("return document.querySelector('#devices img, #invitations img, #transfers img')")
    assert source.name in text("#transfers")
    print("PASS: malicious device names and filenames display as text", flush=True)

    click("#show-more")
    assert not js("return document.querySelector('[data-settings], #setting-port, #setting-networks, #setting-invitation, #setting-idle, #settings-toml, #problems')")
    assert not js("return document.querySelector('#settings-content').hidden")
    assert js("return [...document.querySelectorAll('[name=provider]')].length") == 4
    assert "restart required" in text("#settings-form")
    js("window.tomlError=null; act('save_settings',{toml:'prot = 1234'}).catch(error=>window.tomlError=String(error)); return true")
    wait(lambda: "config.toml" in text("#error") and "prot" in text("#error"))
    assert "port = 19843" in (app_dir / "config.toml").read_text()
    print("PASS: config-only validation preserves saved configuration", flush=True)
    fill("#setting-folder", str(app_received))
    click("#settings-form button:not(#open-folder)")
    wait(lambda: "Saved." in text("#settings-status"))
    assert "port = 19843" in (app_dir / "config.toml").read_text()
    click("#show-send")
    print("PASS: single-level Settings exposes approved controls and preserves config-only values", flush=True)
    # Native WebDriver key input exercises keyboard activation and modal Escape.
    js("document.querySelector('#show-more').focus(); return true")
    http("POST", f"/session/{session}/actions", {"actions":[{"type":"key","id":"keyboard","actions":[{"type":"keyDown","value":"\ue007"},{"type":"keyUp","value":"\ue007"}]}]})
    wait(lambda: not js("return document.querySelector('#more-page').hidden"))
    js("document.querySelector('#show-send').focus(); return true")
    http("POST", f"/session/{session}/actions", {"actions":[{"type":"key","id":"keyboard","actions":[{"type":"keyDown","value":"\ue007"},{"type":"keyUp","value":"\ue007"}]}]})
    wait(lambda: js("return document.querySelector('#more-page').hidden"))
    assert js("return document.activeElement.id") == "show-more"
    print("PASS: keyboard-only Settings activation and return focus", flush=True)
    js("document.querySelector('#pair-confirm').disabled = false; document.querySelector('#outgoing-pair').showModal(); document.querySelector('#pair-dismiss').focus(); return true")
    http("POST", f"/session/{session}/actions", {"actions":[{"type":"key","id":"keyboard","actions":[{"type":"keyDown","value":"\ue004"},{"type":"keyUp","value":"\ue004"}]}]})
    assert js("return document.activeElement.closest('dialog')?.id") == "outgoing-pair"
    http("POST", f"/session/{session}/actions", {"actions":[{"type":"key","id":"keyboard","actions":[{"type":"keyDown","value":"\ue00c"},{"type":"keyUp","value":"\ue00c"}]}]})
    assert not js("return document.querySelector('#outgoing-pair').open")
    print("PASS: modal keyboard focus containment and Escape cancellation", flush=True)
    js("document.querySelector('#selected-files button').click(); return true")
    wait(lambda: not text("#selected-files"))
    assert js("return chosenFiles.length") == 0
    print("PASS: remove one selected file", flush=True)
    js("window.__TAURI__.event.emit('tauri://drag-drop',{paths:[arguments[0]]}); return true", [str(source)])
    time.sleep(0.4)
    assert js("return chosenFiles.length") == 0
    print("PASS: drag-drop cannot bypass the plus-only picker flow", flush=True)
    print("21 UI scenarios passed; 0 failed", flush=True)


if __name__ == "__main__":
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
