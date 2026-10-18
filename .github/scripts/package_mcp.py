"""Package the native connector and exercise its real stdio protocol offline."""
import json
import os
from pathlib import Path
import queue
import subprocess
import sys
import tempfile
import threading
import zipfile


def smoke(executable):
    process = subprocess.Popen(
        [str(executable.resolve())], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
    )
    messages = queue.Queue()

    def read():
        for line in process.stdout:
            messages.put(json.loads(line))

    threading.Thread(target=read, daemon=True).start()

    def send(payload):
        process.stdin.write(json.dumps(payload) + "\n")
        process.stdin.flush()

    try:
        send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-11-25", "capabilities": {},
            "clientInfo": {"name": "Plotium package smoke", "version": "1"},
        }})
        assert messages.get(timeout=10)["result"]["serverInfo"]["name"] == "Plotium"
        send({"jsonrpc": "2.0", "method": "notifications/initialized"})
        send({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}})
        tools = messages.get(timeout=10)["result"]["tools"]
        assert len(tools) == 11
        for tool in tools:
            if tool["name"] not in ("plotium_get_state", "plotium_execute", "plotium_properties_list"):
                assert "expected_project_context" in tool["inputSchema"]["required"]
    finally:
        process.stdin.close()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)
    assert process.returncode == 0


def package(directory, platform, version):
    suffix = ".exe" if platform.startswith("windows-") else ""
    binary = directory / ("plotium-mcp" + suffix)
    binary.chmod(0o755)
    smoke(binary)
    if not platform.startswith(("macos-", "windows-")):
        return
    manifest = {
        "manifest_version": "0.3", "name": "plotium", "display_name": "Plotium",
        "version": version, "description": "Connect Claude Desktop to the open Plotium project.",
        "author": {"name": "Jiekang Tian"}, "license": "GPL-3.0-or-later",
        "homepage": "https://plotium.nmrtist.space/",
        "server": {"type": "binary", "entry_point": "server/" + binary.name,
                   "mcp_config": {"command": "${__dirname}/server/" + binary.name,
                                  "args": ["--client", "claude-desktop"]}},
        "compatibility": {"platforms": ["win32" if suffix else "darwin"]},
    }
    bundle = directory / "plotium.mcpb"
    with zipfile.ZipFile(bundle, "w", zipfile.ZIP_DEFLATED) as output:
        output.writestr("manifest.json", json.dumps(manifest, indent=2))
        output.write(binary, "server/" + binary.name)
        output.write(directory / "LICENSE", "LICENSE")
        output.write(directory / "MCP-THIRD-PARTY-LICENSES.html", "THIRD-PARTY-LICENSES.html")
    with zipfile.ZipFile(bundle) as output:
        assert output.testzip() is None
        assert json.loads(output.read("manifest.json"))["server"]["type"] == "binary"
        assert output.read("server/" + binary.name) == binary.read_bytes()
        with tempfile.TemporaryDirectory(prefix="plotium-mcpb-smoke-") as temporary:
            extracted = Path(temporary) / binary.name
            extracted.write_bytes(output.read("server/" + binary.name))
            extracted.chmod(0o755)
            smoke(extracted)


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "smoke":
        smoke(Path(sys.argv[2]))
    else:
        package(Path("dist/package"), os.environ["PLATFORM"], os.environ["VERSION"])
