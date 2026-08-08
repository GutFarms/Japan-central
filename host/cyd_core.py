"""Shared metrics collection and CYD link helpers for CLI + desktop app."""

from __future__ import annotations

import json
import socket
import sys
import time
import warnings
from dataclasses import dataclass
from typing import Any, Optional

import psutil

try:
    import serial  # type: ignore
    from serial.tools import list_ports

    _HAS_SERIAL = True
except Exception:  # noqa: BLE001
    serial = None  # type: ignore
    list_ports = None  # type: ignore
    _HAS_SERIAL = False

try:
    with warnings.catch_warnings():
        warnings.simplefilter("ignore", FutureWarning)
        from pynvml import (
            NVML_TEMPERATURE_GPU,
            nvmlDeviceGetHandleByIndex,
            nvmlDeviceGetMemoryInfo,
            nvmlDeviceGetName,
            nvmlDeviceGetTemperature,
            nvmlDeviceGetUtilizationRates,
            nvmlInit,
            nvmlShutdown,
        )

    _HAS_NVML = True
except Exception:  # noqa: BLE001
    _HAS_NVML = False


# Common USB-UART chips used on ESP32-CYD boards (VID, PID).
_CYD_USB_IDS = {
    (0x10C4, 0xEA60),  # CP2102
    (0x10C4, 0xEA70),
    (0x1A86, 0x7523),  # CH340
    (0x1A86, 0x55D4),  # CH9102
    (0x0403, 0x6001),  # FTDI
    (0x0403, 0x6015),
    (0x303A, 0x1001),  # Espressif
    (0x303A, 0x0002),
}

_CYD_KEYWORDS = (
    "cp210",
    "ch340",
    "ch910",
    "ftdi",
    "usb-serial",
    "usb serial",
    "uart",
    "esp32",
    "silicon labs",
    "usb_serial",
)


def clamp_pct(value: float) -> float:
    return max(0.0, min(100.0, float(value)))


def cpu_temperature_c() -> float:
    try:
        temps = psutil.sensors_temperatures(fahrenheit=False)
    except Exception:  # noqa: BLE001
        return 0.0
    if not temps:
        return 0.0
    for key in ("coretemp", "k10temp", "cpu_thermal", "acpitz", "zenpower"):
        entries = temps.get(key)
        if entries and entries[0].current is not None:
            return float(entries[0].current)
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


@dataclass(frozen=True)
class SerialPortInfo:
    device: str
    description: str
    vid: Optional[int]
    pid: Optional[int]
    score: int

    @property
    def label(self) -> str:
        chip = self.description or "Serial"
        return f"{self.device} — {chip}"


def _looks_like_platform_uart(device: str) -> bool:
    name = device.rsplit("/", 1)[-1].lower()
    return name.startswith("ttys") or name.startswith("ttyama") or name.startswith("ttyprintk")


def list_serial_port_infos(include_platform: bool = False) -> list[SerialPortInfo]:
    if not _HAS_SERIAL:
        return []
    found: list[SerialPortInfo] = []
    for p in list_ports.comports():
        if not include_platform and _looks_like_platform_uart(p.device):
            continue
        desc = (p.description or "") + " " + (p.manufacturer or "")
        desc_l = desc.lower()
        vid = int(p.vid) if p.vid is not None else None
        pid = int(p.pid) if p.pid is not None else None
        score = 0
        if vid is not None and pid is not None and (vid, pid) in _CYD_USB_IDS:
            score += 100
        if vid == 0x303A:
            score += 80
        dev = p.device.lower()
        if "ttyusb" in dev or "ttyacm" in dev or dev.startswith("com"):
            score += 15
        for kw in _CYD_KEYWORDS:
            if kw in desc_l:
                score += 20
                break
        if "bluetooth" in desc_l or "debug" in desc_l:
            score -= 50
        found.append(
            SerialPortInfo(
                device=p.device,
                description=(p.description or "Serial").strip(),
                vid=vid,
                pid=pid,
                score=score,
            )
        )
    found.sort(key=lambda x: (-x.score, x.device))
    return found


def list_serial_ports() -> list[str]:
    return [p.device for p in list_serial_port_infos()]


def pick_best_port(preferred: Optional[str] = None) -> Optional[SerialPortInfo]:
    ports = list_serial_port_infos()
    if not ports:
        return None
    if preferred:
        for p in ports:
            if p.device == preferred:
                return p
    ranked = [p for p in ports if p.score > 0]
    return ranked[0] if ranked else ports[0]


class SerialTransport:
    def __init__(self, port: str, baud: int = 115200, settle_s: float = 1.5) -> None:
        if not _HAS_SERIAL:
            raise RuntimeError("pyserial is required for USB serial")
        self.port = port
        self.baud = baud
        self.ser = serial.Serial(port=port, baudrate=baud, timeout=0.2)
        time.sleep(max(0.0, settle_s))
        self.ser.reset_input_buffer()

    def send(self, data: bytes) -> None:
        payload = data if data.endswith(b"\n") else data + b"\n"
        self.ser.write(payload)
        self.ser.flush()

    def close(self) -> None:
        try:
            self.ser.close()
        except Exception:  # noqa: BLE001
            pass

    def describe(self) -> str:
        return f"USB serial {self.port} @ {self.baud}"

    @property
    def is_open(self) -> bool:
        try:
            return bool(self.ser and self.ser.is_open)
        except Exception:  # noqa: BLE001
            return False


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


def encode_metrics(payload: dict[str, Any]) -> bytes:
    return json.dumps(payload, separators=(",", ":")).encode("utf-8")
