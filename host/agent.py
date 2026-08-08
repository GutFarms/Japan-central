#!/usr/bin/env python3
"""PC / GPU metrics agent for the ESP32-CYD monitor.

Collects CPU, RAM, and NVIDIA GPU stats and sends JSON UDP packets
to the firmware (see protocol/metrics_v1.md).
"""

from __future__ import annotations

import argparse
import json
import socket
import sys
import time
from typing import Any

import psutil

try:
    from pynvml import (
        nvmlDeviceGetHandleByIndex,
        nvmlDeviceGetName,
        nvmlDeviceGetTemperature,
        nvmlDeviceGetUtilizationRates,
        nvmlDeviceGetMemoryInfo,
        nvmlInit,
        nvmlShutdown,
        NVML_TEMPERATURE_GPU,
    )

    _HAS_NVML = True
except Exception:  # noqa: BLE001 - optional dependency / missing driver
    _HAS_NVML = False


def clamp_pct(value: float) -> float:
    return max(0.0, min(100.0, float(value)))


def cpu_temperature_c() -> float:
    """Best-effort CPU temperature (°C). Returns 0 if unavailable."""
    try:
        temps = psutil.sensors_temperatures(fahrenheit=False)
    except Exception:  # noqa: BLE001
        return 0.0

    if not temps:
        return 0.0

    preferred_keys = ("coretemp", "k10temp", "cpu_thermal", "acpitz", "zenpower")
    for key in preferred_keys:
        entries = temps.get(key)
        if entries:
            reading = entries[0].current
            if reading is not None:
                return float(reading)

    for entries in temps.values():
        if entries and entries[0].current is not None:
            return float(entries[0].current)
    return 0.0


class GpuReader:
    def __init__(self, index: int = 0) -> None:
        self.enabled = False
        self.handle = None
        self.name = ""
        if not _HAS_NVML:
            return
        try:
            nvmlInit()
            self.handle = nvmlDeviceGetHandleByIndex(index)
            raw = nvmlDeviceGetName(self.handle)
            self.name = raw.decode() if isinstance(raw, bytes) else str(raw)
            self.enabled = True
        except Exception as exc:  # noqa: BLE001
            print(f"[warn] NVIDIA GPU unavailable: {exc}", file=sys.stderr)
            self.enabled = False

    def read(self) -> dict[str, float]:
        if not self.enabled or self.handle is None:
            return {"gpu": 0.0, "gpu_temp": 0.0, "vram": 0.0}
        try:
            util = nvmlDeviceGetUtilizationRates(self.handle)
            mem = nvmlDeviceGetMemoryInfo(self.handle)
            temp = nvmlDeviceGetTemperature(self.handle, NVML_TEMPERATURE_GPU)
            vram_pct = (float(mem.used) / float(mem.total) * 100.0) if mem.total else 0.0
            return {
                "gpu": clamp_pct(util.gpu),
                "gpu_temp": float(temp),
                "vram": clamp_pct(vram_pct),
            }
        except Exception as exc:  # noqa: BLE001
            print(f"[warn] GPU read failed: {exc}", file=sys.stderr)
            return {"gpu": 0.0, "gpu_temp": 0.0, "vram": 0.0}

    def close(self) -> None:
        if self.enabled:
            try:
                nvmlShutdown()
            except Exception:  # noqa: BLE001
                pass


def collect_metrics(gpu: GpuReader, host_name: str) -> dict[str, Any]:
    cpu = clamp_pct(psutil.cpu_percent(interval=None))
    ram = clamp_pct(psutil.virtual_memory().percent)
    gpu_stats = gpu.read()
    return {
        "v": 1,
        "cpu": round(cpu, 1),
        "cpu_temp": round(cpu_temperature_c(), 1),
        "ram": round(ram, 1),
        "gpu": round(gpu_stats["gpu"], 1),
        "gpu_temp": round(gpu_stats["gpu_temp"], 1),
        "vram": round(gpu_stats["vram"], 1),
        "fps": 0,
        "host": host_name[:23],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Send PC/GPU metrics to ESP32-CYD")
    parser.add_argument("--host", required=True, help="CYD IP address on the LAN")
    parser.add_argument("--port", type=int, default=4210, help="UDP port (default 4210)")
    parser.add_argument("--interval", type=float, default=0.5, help="Send interval seconds")
    parser.add_argument(
        "--name",
        default=socket.gethostname(),
        help="Short host label shown on the display",
    )
    parser.add_argument("--gpu-index", type=int, default=0, help="NVIDIA GPU index")
    parser.add_argument(
        "--once",
        action="store_true",
        help="Send a single packet and exit (useful for smoke tests)",
    )
    args = parser.parse_args()

    # Prime cpu_percent so the first real sample is meaningful.
    psutil.cpu_percent(interval=None)

    gpu = GpuReader(index=args.gpu_index)
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)

    print(f"Sending metrics → {args.host}:{args.port} every {args.interval}s")
    if gpu.enabled:
        print(f"GPU: {gpu.name}")
    else:
        print("GPU: (none / non-NVIDIA) — GPU fields will be 0")

    try:
        while True:
            payload = collect_metrics(gpu, args.name)
            data = json.dumps(payload, separators=(",", ":")).encode("utf-8")
            sock.sendto(data, (args.host, args.port))
            print(
                f"cpu={payload['cpu']:5.1f}%  gpu={payload['gpu']:5.1f}%  "
                f"ram={payload['ram']:5.1f}%  vram={payload['vram']:5.1f}%",
                end="\r",
                flush=True,
            )
            if args.once:
                print()
                break
            time.sleep(max(0.1, args.interval))
    except KeyboardInterrupt:
        print("\nStopped.")
    finally:
        sock.close()
        gpu.close()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
