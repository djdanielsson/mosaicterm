"""Probe a private Rex server for MosaicTerm-compatible command history.

Usage: python3 examples/rex_backend_probe.py /path/to/rex
Tested with Rex 0.1.0-preview.21 on Linux.
"""

import json
import os
import shlex
import subprocess
import sys
import tempfile
import time
from pathlib import Path


def wait_for(read, predicate):
    deadline = time.monotonic() + 5
    while True:
        value = read()
        if predicate(value):
            return value
        if time.monotonic() >= deadline:
            raise RuntimeError(f"Timed out waiting for Rex output: {value!r}")
        time.sleep(0.05)


def probe(rex, root):
    socket = root / "rex.sock"
    env = {
        **os.environ,
        "XDG_CONFIG_HOME": str(root / "config"),
        "XDG_STATE_HOME": str(root / "state"),
        "XDG_CACHE_HOME": str(root / "cache"),
        "TERM": "xterm-256color",
    }
    cli = [
        rex, "--server", str(socket), "--autostart=false", "--timeout", "3s",
    ]

    def run(*args):
        return subprocess.check_output([*cli, *args], env=env, text=True)

    def api(method, **args):
        return json.loads(run("api", "call", method, json.dumps(args)))

    events = []
    with (root / "server.log").open("w") as log:
        server = subprocess.Popen(
            [
                rex, "server", "run", "--listen", str(socket),
                "--state-dir", str(root / "server"), "--restore=false",
            ],
            env=env,
            stdout=log,
            stderr=log,
        )
        try:
            wait_for(socket.exists, bool)
            version = api("server.version")
            print(f"Rex {version['rex_version']}, API revision {version['api_revision']}")
            created = json.loads(
                run(
                    "new", "--json", "--keep-open", "--cwd", str(root), "--shell",
                    "none", "probe", "--", "/bin/bash", "--noprofile", "--norc",
                )
            )
            session = created["session_id"]
            block = created["initial_windows"][0]["block_ids"][0]
            target = {"session_id": session, "block_id": block}

            def subscribe(path):
                with path.open("w") as output:
                    client = subprocess.Popen(
                        [*cli, "events", "-s", session, "--json"],
                        env=env,
                        stdout=output,
                        stderr=subprocess.DEVNULL,
                    )
                events.append(client)
                return client

            def send(command):
                run("send", "-s", session, "-b", block, command)
                run("send-key", "-s", session, "-b", block, "Enter")

            def snapshot(output_format="text"):
                return api(
                    "com.superlogical.terminal.format",
                    **target,
                    args={"format": output_format},
                )["content"]

            listed = json.loads(run("ls", "--json"))["sessions"]
            assert any(entry["session_id"] == session for entry in listed)
            assert api("session.attach", session_id=session)["view"]["session_id"] == session
            ticket = api(
                "data.connect",
                **target,
                data_ref_id="terminal",
                subprotocol="rex.data.terminal.v1",
            )
            assert ticket["ticket"] and ticket["expires_in_ms"] > 0
            api("com.superlogical.terminal.resize", **target, args={"columns": 106, "rows": 37})
            first_log = root / "first.jsonl"
            first_client = subscribe(first_log)
            cwd = root / "cwd"
            cwd.mkdir()
            time.sleep(0.15)
            send(
                "cd cwd; printf '\\033]7;%s\\007' " + shlex.quote(cwd.as_uri())
                + "; printf '\\nsize:'; stty size; printf '\\npid:%s\\n' \"$$\"; "
                + "false; printf '\\033]133;D;%s\\007' \"$?\""
            )
            screen = wait_for(
                snapshot,
                lambda text: "size:37 106" in text
                and any(line.startswith("pid:") for line in text.splitlines()),
            )
            pid = next(line.strip()[4:] for line in screen.splitlines() if line.startswith("pid:"))
            assert "child_exited" not in wait_for(first_log.read_text, lambda text: "pwd_changed" in text)

            send("for i in $(seq 1 20); do printf 'tick:%02d\\n' \"$i\"; sleep .10; done")
            wait_for(snapshot, lambda text: "tick:01" in text)
            first_client.terminate()
            first_client.wait(timeout=3)
            time.sleep(0.45)
            second_log = root / "second.jsonl"
            subscribe(second_log)
            assert api("session.attach", session_id=session)["view"]["session_id"] == session
            wait_for(snapshot, lambda text: "tick:20" in text)
            send("printf '\\nreattached:%s\\n' \"$$\"; pwd; true; printf '\\033]133;D;%s\\007' \"$?\"")
            restored = wait_for(snapshot, lambda text: f"reattached:{pid}" in text and str(cwd) in text)
            assert "tick:04" in restored
            assert "\x1b]133;" not in snapshot("vt")
            assert "child_exited" not in second_log.read_text()
            send("exit 9")
            exit_event = wait_for(second_log.read_text, lambda text: "child_exited" in text)
            assert '"exit_code":9' in exit_event.replace(" ", "")
            print("Verified list, attach, input, resize, directory events, and a data ticket.")
            print("The same shell retained recent output across reconnect.")
            print("Shell exit produced child_exited(9). The restored VT snapshot omitted OSC 133.")
        finally:
            for client in events:
                if client.poll() is None:
                    client.terminate()
                    client.wait(timeout=3)
            if server.poll() is None:
                server.terminate()
                server.wait(timeout=5)


if __name__ == "__main__":
    executable = sys.argv[1] if len(sys.argv) > 1 else "rex"
    with tempfile.TemporaryDirectory(prefix="mosaicterm-rex-probe-") as directory:
        probe(executable, Path(directory))
