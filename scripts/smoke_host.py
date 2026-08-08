#!/usr/bin/env python3
"""Offline smoke checks for the host agent protocol helpers."""

from __future__ import annotations

import json
import socket
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
AGENT = ROOT / "host" / "agent.py"
VENV_PY = ROOT / "host" / ".venv" / "bin" / "python"
PY = str(VENV_PY if VENV_PY.exists() else Path(sys.executable))


def main() -> int:
    sys.path.insert(0, str(ROOT / "host"))
    from agent import GpuReader, collect_metrics  # noqa: E402

    gpu = GpuReader()
    payload = collect_metrics(gpu, "SMOKE")
    gpu.close()

    required = {"v", "cpu", "cpu_temp", "ram", "gpu", "gpu_temp", "vram", "fps", "host"}
    missing = required - set(payload)
    assert not missing, f"missing keys: {missing}"
    assert payload["v"] == 1
    assert payload["host"] == "SMOKE"
    assert 0.0 <= payload["cpu"] <= 100.0
    assert 0.0 <= payload["ram"] <= 100.0

    # NDJSON framing: one object + newline, under 512 bytes.
    line = json.dumps(payload, separators=(",", ":")).encode("utf-8") + b"\n"
    assert len(line) <= 512, f"packet too large: {len(line)}"
    assert line.endswith(b"\n")

    # UDP round-trip through the real agent CLI.
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(("127.0.0.1", 0))
    sock.settimeout(5)
    port = sock.getsockname()[1]
    proc = subprocess.Popen(
        [PY, str(AGENT), "--host", "127.0.0.1", "--port", str(port), "--once", "--name", "SMOKE"],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    data, _addr = sock.recvfrom(512)
    proc.wait(timeout=5)
    sock.close()
    rx = json.loads(data.decode("utf-8"))
    assert rx["v"] == 1 and rx["host"] == "SMOKE", rx
    print("smoke_host: OK", json.dumps(rx, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
