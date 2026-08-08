#!/usr/bin/env python3
"""PC / GPU metrics agent for the ESP32-CYD monitor.

Collects CPU, RAM, and NVIDIA GPU stats and sends JSON over USB serial
(NDJSON) and/or UDP (see protocol/metrics_v1.md).
"""

from __future__ import annotations

import argparse
import json
import socket
import sys
import time
from typing import Any, Optional, Protocol

import psutil

try:
    import serial  # type: ignore
    from serial.tools import list_ports

    _HAS_SERIAL = True
except Exception:  # noqa: BLE001
    _HAS_SERIAL = False

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


class Transport(Protocol):
    def send(self, data: bytes) -> None: ...
    def close(self) -> None: ...
    def describe(self) -> str: ...


class UdpTransport:
    def __init__(self, host: str, port: int) -> None:
        self.host = host
        self.port = port
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)

    def send(self, data: bytes) -> None:
        self.sock.sendto(data, (self.host, self.port))

    def close(self) -> None:
        self.sock.close()

    def describe(self) -> str:
        return f"UDP {self.host}:{self.port}"


class SerialTransport:
    def __init__(self, port: str, baud: int = 115200) -> None:
        if not _HAS_SERIAL:
            raise RuntimeError("pyserial is required for --serial (pip install pyserial)")
        self.port = port
        self.baud = baud
        self.ser = serial.Serial(port=port, baudrate=baud, timeout=0.2)
        # Give the ESP32 a moment after the port opens (some boards reset on open).
        time.sleep(1.5)
        self.ser.reset_input_buffer()

    def send(self, data: bytes) -> None:
        self.ser.write(data if data.endswith(b"\n") else data + b"\n")
        self.ser.flush()

    def close(self) -> None:
        self.ser.close()

    def describe(self) -> str:
        return f"USB serial {self.port} @ {self.baud}"


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


def list_serial_ports() -> list[str]:
    if not _HAS_SERIAL:
        return []
    return [p.device for p in list_ports.comports()]


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Send PC/GPU metrics to ESP32-CYD over USB serial and/or UDP"
    )
    parser.add_argument(
        "--serial",
        metavar="PORT",
        help="USB serial device (e.g. COM3, /dev/ttyUSB0). Use 'auto' to pick the first port.",
    )
    parser.add_argument("--baud", type=int, default=115200, help="Serial baud rate")
    parser.add_argument("--host", help="CYD IP address for UDP mode")
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
    parser.add_argument(
        "--list-ports",
        action="store_true",
        help="List available serial ports and exit",
    )
    args = parser.parse_args()

    if args.list_ports:
        ports = list_serial_ports()
        if not ports:
            print("No serial ports found.")
            return 1
        for p in ports:
            print(p)
        return 0

    if not args.serial and not args.host:
        parser.error("Provide --serial PORT and/or --host IP (or --list-ports)")

    transports: list[Transport] = []

    if args.serial:
        port = args.serial
        if port.lower() == "auto":
            ports = list_serial_ports()
            if not ports:
                print("No serial ports found for --serial auto", file=sys.stderr)
                return 1
            port = ports[0]
            print(f"Auto-selected serial port: {port}")
        transports.append(SerialTransport(port, baud=args.baud))

    if args.host:
        transports.append(UdpTransport(args.host, args.port))

    # Prime cpu_percent so the first real sample is meaningful.
    psutil.cpu_percent(interval=None)

    gpu = GpuReader(index=args.gpu_index)
    for t in transports:
        print(f"Sending metrics → {t.describe()} every {args.interval}s")
    if gpu.enabled:
        print(f"GPU: {gpu.name}")
    else:
        print("GPU: (none / non-NVIDIA) — GPU fields will be 0")

    try:
        while True:
            payload = collect_metrics(gpu, args.name)
            data = json.dumps(payload, separators=(",", ":")).encode("utf-8")
            for t in transports:
                t.send(data)
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
        for t in transports:
            t.close()
        gpu.close()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
