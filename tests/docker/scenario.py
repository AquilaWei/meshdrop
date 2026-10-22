"""Three isolated multicast peers; real CLI pairing, transfer and rejection."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import uuid

engine = os.environ.get("CONTAINER_ENGINE", "podman")
network = "meshdrop-" + uuid.uuid4().hex[:10]
containers = []

def run(*args, input=None, check=True):
    result = subprocess.run([engine, *args], input=input, text=True, capture_output=True)
    if check and result.returncode:
        raise RuntimeError(f"{args}: {result.stdout}\n{result.stderr}")
    return result

def execute(container, *args, **kwargs):
    return run("exec", "-i", container, *args, **kwargs)

def wait_receiver(container):
    for _ in range(100):
        result = execute(container, "cat", "/state/receiver.log", check=False)
        if "listening" in result.stdout:
            return
        time.sleep(0.1)
    raise RuntimeError("receiver startup failed: " + result.stdout + result.stderr)

try:
    run("network", "create", network)
    with tempfile.TemporaryDirectory(prefix="meshdrop-container-") as temporary:
        root = Path(temporary)
        for index in range(3):
            name = f"{network}-{index}"
            run("run", "-d", "--name", name, "--network", network,
                "docker.io/library/ubuntu:26.04", "sleep", "infinity")
            containers.append(name)
            run("cp", "target/debug/meshdrop", name + ":/usr/local/bin/meshdrop")
            execute(name, "mkdir", "-p", "/state", "/received")
            config = root / f"config-{index}.toml"
            config.write_text(f'name = "peer-{index}"\nreceive_directory = "/received"\n'
                              'providers = ["mdns"]\ntimeout_seconds = 30\n')
            run("cp", str(config), name + ":/state/config.toml")
            flag = "--auto-reject" if index == 2 else "--auto-accept"
            run("exec", "-d", name, "sh", "-c",
                f"printf 'y\\ny\\ny\\n' | meshdrop --state-dir /state receive {flag} >/state/receiver.log 2>&1")
            wait_receiver(name)
        addresses = []
        for container in containers:
            data = json.loads(run("inspect", container).stdout)[0]
            ip = data["NetworkSettings"]["Networks"][network]["IPAddress"]
            addresses.append(ip + ":9843")
        for index, container in enumerate(containers):
            devices = json.loads(execute(container, "meshdrop", "--state-dir", "/state", "devices").stdout)
            names = {device["name"] for device in devices if device["online"]}
            expected = {f"peer-{n}" for n in range(3) if n != index}
            assert expected <= names, (index, names, expected)
        print("PASS: 3 containers discover each other with mDNS", flush=True)
        for target in (1, 2):
            execute(containers[0], "meshdrop", "--state-dir", "/state", "pair",
                    addresses[target], "--advertise", addresses[0], input="y\n")
        print("PASS: explicit mutual pairing with two peers", flush=True)
        payload = root / "payload.txt"
        payload.write_bytes(b"meshdrop three-container transfer\n")
        run("cp", str(payload), containers[0] + ":/payload.txt")
        execute(containers[0], "meshdrop", "--state-dir", "/state", "send", addresses[1], "/payload.txt")
        actual = execute(containers[1], "sha256sum", "/received/payload.txt").stdout.split()[0]
        assert actual == hashlib.sha256(payload.read_bytes()).hexdigest()
        execute(containers[1], "meshdrop", "--state-dir", "/state", "send", addresses[0], "/received/payload.txt")
        assert execute(containers[0], "sha256sum", "/received/payload.txt").stdout.split()[0] == actual
        print("PASS: bidirectional transfer and SHA-256", flush=True)
        rejected = execute(containers[0], "meshdrop", "--state-dir", "/state", "send", addresses[2], "/payload.txt", check=False)
        assert rejected.returncode != 0 and "rejected" in rejected.stderr, rejected
        assert execute(containers[2], "ls", "-A", "/received").stdout == ""
        print("PASS: rejected offer sends no content", flush=True)
        print("4 scenarios passed; 0 failed", flush=True)
finally:
    for container in containers:
        result = run("rm", "-f", container, check=False)
        if result.returncode:
            print("cleanup failed:", result.stderr)
    result = run("network", "rm", network, check=False)
    if result.returncode:
        print("network cleanup failed:", result.stderr)
