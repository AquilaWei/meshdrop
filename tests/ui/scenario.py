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
    js("document.querySelector('#refresh').focus(); window.refreshNode = document.querySelector('#refresh'); refresh(); return true")
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
    click("[data-settings=add]")
    fill("#address", "127.0.0.1:1")
    click("#manual button")
    wait(lambda: "127.0.0.1:1" in text("#problems"))
    assert "127.0.0.1:1" not in text("#devices")
    click("#show-send")
    click("#show-send")
    print("PASS: unresolved/non-meshdrop candidate is diagnostic only", flush=True)

    # WebDriver controls the webview, not native GTK chooser windows. Inject only
    # the chooser's result through the same existing Rust command used by the
    # dialog callback. Transport, state, UI rendering and filesystem remain real.
    source = root / "payload <img src=x onerror=alert(1)>.txt"
    source.write_bytes(b"meshdrop ui payload\n" * 131072)
    # Supply only the native chooser result; tapping with no selection must ask
    # for files and then continue to pairing without another device/send click.
    js("""const original = act;
        const chooserPath = arguments[0];
        window.chooserCalls = 0;
        act = (operation, payload) => {
            if (operation === 'pick_files') {
                window.chooserCalls++;
                return original('select_files', [chooserPath]);
            }
            return original(operation, payload);
        }; return true;""", [str(source)])
    click("#devices button:not(:disabled)")
    wait(lambda: source.name in text("#selected-files"))
    assert js("return window.chooserCalls") == 1
    wait(lambda: js("return document.querySelector('#outgoing-pair').open"))
    code = text("#pair-code")
    assert re.fullmatch(r"\d{6}", code)
    click("#pair-confirm")
    wait(lambda: code in (root / "cli.log").read_text())
    receiver.stdin.write(b"y\n"); receiver.stdin.flush()
    wait(lambda: "Complete" in text("#transfers"))
    assert not js("return document.querySelector('#outgoing-pair').open")
    print("PASS: tapping an unpaired device pairs with matching codes then automatically invites", flush=True)
    wait(lambda: (cli_received / "payload _img src=x onerror=alert(1)_.txt").exists())
    assert digest(source) == digest(cli_received / "payload _img src=x onerror=alert(1)_.txt")
    assert js("return [...document.querySelectorAll('#transfers progress')].some(p => p.value === 100)")
    print("PASS: injected chooser result, app-to-CLI send, 100% progress and SHA-256", flush=True)

    wait(lambda: "Paired" in text("#devices"))
    click("#devices button:not(:disabled)")
    wait(lambda: js("return document.querySelectorAll('#transfers .transfer').length === 2"))
    assert not js("return document.querySelector('#outgoing-pair').open")
    wait(lambda: js("return [...document.querySelectorAll('.transfer-status')].filter(n => n.textContent === 'Complete').length === 2"))
    print("PASS: tapping an already paired target sends again without a code prompt", flush=True)

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
    click("#invitations [data-action=reject]")
    assert rejecting.wait(timeout=20) != 0
    assert not (app_received / rejected.name).exists()
    assert not list(app_received.glob("*.part"))
    print("PASS: in-app rejection leaves no content or temporary files", flush=True)

    assert peer_name in text("#devices")
    assert not js("return document.querySelector('#devices img, #invitations img, #transfers img')")
    assert source.name in text("#transfers")
    print("PASS: malicious device names and filenames display as text", flush=True)

    click("#show-more")
    click("[data-settings=advanced]")
    js("document.querySelector('#settings-toml').closest('details').open = true; return true")
    fill("#settings-toml", "prot = 1234")
    click("#save-toml")
    wait(lambda: "config.toml" in text("#error") and "prot" in text("#error"))
    assert "port = 19843" in (app_dir / "config.toml").read_text()
    print("PASS: invalid TOML reports path and preserves saved settings", flush=True)
    click("#show-send")
    assert text("#settings-title") == "Settings"
    click("[data-settings=receiving]")
    assert text("#settings-title") == "Receiving"
    fill("#setting-port", "19843")
    click("#settings-form button")
    wait(lambda: "Saved." in text("#settings-status"))
    assert "port = 19843" in (app_dir / "config.toml").read_text()
    click("#show-send")
    click("#show-send")
    print("PASS: Settings level navigation, back and save", flush=True)
    # Native WebDriver key input exercises keyboard activation and modal Escape.
    js("document.querySelector('#show-more').focus(); return true")
    http("POST", f"/session/{session}/actions", {"actions":[{"type":"key","id":"keyboard","actions":[{"type":"keyDown","value":"\ue007"},{"type":"keyUp","value":"\ue007"}]}]})
    wait(lambda: not js("return document.querySelector('#more-page').hidden"))
    js("document.querySelector('#show-send').focus(); return true")
    http("POST", f"/session/{session}/actions", {"actions":[{"type":"key","id":"keyboard","actions":[{"type":"keyDown","value":"\ue007"},{"type":"keyUp","value":"\ue007"}]}]})
    wait(lambda: js("return document.querySelector('#more-page').hidden"))
    assert js("return document.activeElement.id") == "show-more"
    print("PASS: keyboard-only Settings activation and return focus", flush=True)
    js("document.querySelector('#selected-files button').click(); return true")
    wait(lambda: not text("#selected-files"))
    assert js("return chosenFiles.length") == 0
    print("PASS: remove one selected file", flush=True)
    print("14 UI scenarios passed; 0 failed", flush=True)


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
