#!/usr/bin/env python3
"""CYD Monitor desktop app — auto-detects ESP32-CYD over USB and streams metrics."""

from __future__ import annotations

import socket
import threading
import time
import tkinter as tk
from tkinter import ttk
from typing import Any, Optional

import psutil

from cyd_core import (
    GpuReader,
    SerialTransport,
    collect_metrics,
    encode_metrics,
    list_serial_port_infos,
    pick_best_port,
)

APP_TITLE = "CYD Monitor"
INTERVAL_S = 0.5
RECONNECT_S = 2.0
BAUD = 115200


class MonitorApp(tk.Tk):
    def __init__(self) -> None:
        super().__init__()
        self.title(APP_TITLE)
        self.geometry("420x360")
        self.minsize(380, 320)
        self.configure(bg="#0A1930")

        self._stop = threading.Event()
        self._worker: Optional[threading.Thread] = None
        self._transport: Optional[SerialTransport] = None
        self._gpu = GpuReader()
        self._host_name = socket.gethostname()[:23]
        self._lock = threading.Lock()

        self.status_var = tk.StringVar(value="Starting…")
        self.port_var = tk.StringVar(value="—")
        self.gpu_var = tk.StringVar(value=self._gpu.name or "GPU not detected")
        self.cpu_var = tk.StringVar(value="CPU  —")
        self.ram_var = tk.StringVar(value="RAM  —")
        self.gpu_load_var = tk.StringVar(value="GPU  —")
        self.vram_var = tk.StringVar(value="VRAM —")
        self.auto_var = tk.BooleanVar(value=True)

        self._build_style()
        self._build_ui()
        self.protocol("WM_DELETE_WINDOW", self._on_close)

        psutil.cpu_percent(interval=None)
        self.after(200, self._start_worker)

    def _build_style(self) -> None:
        style = ttk.Style(self)
        try:
            style.theme_use("clam")
        except tk.TclError:
            pass
        style.configure("Root.TFrame", background="#0A1930")
        style.configure("Card.TFrame", background="#122A69")
        style.configure(
            "Title.TLabel",
            background="#0A1930",
            foreground="#5CB8FF",
            font=("Segoe UI", 18, "bold"),
        )
        style.configure(
            "Body.TLabel",
            background="#122A69",
            foreground="#E8F1FF",
            font=("Segoe UI", 11),
        )
        style.configure(
            "Muted.TLabel",
            background="#122A69",
            foreground="#8FB4E8",
            font=("Segoe UI", 10),
        )
        style.configure(
            "Status.TLabel",
            background="#0A1930",
            foreground="#7DFFB2",
            font=("Segoe UI", 11, "bold"),
        )
        style.configure(
            "Accent.TButton",
            background="#2F6FED",
            foreground="#FFFFFF",
            font=("Segoe UI", 10, "bold"),
            padding=8,
        )
        style.map("Accent.TButton", background=[("active", "#4D8CFF")])

    def _build_ui(self) -> None:
        root = ttk.Frame(self, style="Root.TFrame", padding=16)
        root.pack(fill=tk.BOTH, expand=True)

        ttk.Label(root, text="CYD Monitor", style="Title.TLabel").pack(anchor=tk.W)
        ttk.Label(
            root,
            text="Auto-connects over USB and streams PC / GPU stats",
            style="Status.TLabel",
        ).pack(anchor=tk.W, pady=(4, 12))

        card = ttk.Frame(root, style="Card.TFrame", padding=14)
        card.pack(fill=tk.BOTH, expand=True)

        ttk.Label(card, textvariable=self.status_var, style="Body.TLabel").pack(anchor=tk.W)
        ttk.Label(card, textvariable=self.port_var, style="Muted.TLabel").pack(
            anchor=tk.W, pady=(4, 10)
        )
        ttk.Label(card, textvariable=self.gpu_var, style="Muted.TLabel").pack(anchor=tk.W)

        sep = ttk.Separator(card, orient=tk.HORIZONTAL)
        sep.pack(fill=tk.X, pady=12)

        for var in (self.cpu_var, self.gpu_load_var, self.ram_var, self.vram_var):
            ttk.Label(card, textvariable=var, style="Body.TLabel").pack(anchor=tk.W, pady=2)

        btns = ttk.Frame(root, style="Root.TFrame")
        btns.pack(fill=tk.X, pady=(14, 0))

        ttk.Checkbutton(
            btns,
            text="Auto-reconnect",
            variable=self.auto_var,
        ).pack(side=tk.LEFT)

        ttk.Button(btns, text="Rescan USB", style="Accent.TButton", command=self._rescan).pack(
            side=tk.RIGHT
        )

    def _set_status(self, text: str) -> None:
        self.after(0, lambda: self.status_var.set(text))

    def _set_port(self, text: str) -> None:
        self.after(0, lambda: self.port_var.set(text))

    def _set_metrics(self, payload: dict[str, Any]) -> None:
        def apply() -> None:
            self.cpu_var.set(f"CPU   {payload['cpu']:5.1f}%   {payload['cpu_temp']:.0f}°C")
            self.gpu_load_var.set(
                f"GPU   {payload['gpu']:5.1f}%   {payload['gpu_temp']:.0f}°C"
            )
            self.ram_var.set(f"RAM   {payload['ram']:5.1f}%")
            self.vram_var.set(f"VRAM  {payload['vram']:5.1f}%")

        self.after(0, apply)

    def _start_worker(self) -> None:
        if self._worker and self._worker.is_alive():
            return
        self._stop.clear()
        self._worker = threading.Thread(target=self._run_loop, name="cyd-link", daemon=True)
        self._worker.start()

    def _rescan(self) -> None:
        with self._lock:
            if self._transport is not None:
                self._transport.close()
                self._transport = None
        self._set_status("Rescanning USB…")
        self._set_port("—")

    def _connect(self) -> bool:
        best = pick_best_port()
        if best is None:
            ports = list_serial_port_infos()
            self._set_status("Waiting for ESP32-CYD USB…")
            self._set_port("No serial ports found" if not ports else "No CYD-like port yet")
            return False
        try:
            transport = SerialTransport(best.device, baud=BAUD, settle_s=1.2)
        except Exception as exc:  # noqa: BLE001
            self._set_status(f"Open failed: {exc}")
            self._set_port(best.label)
            return False
        with self._lock:
            self._transport = transport
        self._set_status("Linked — streaming metrics")
        self._set_port(best.label)
        return True

    def _run_loop(self) -> None:
        while not self._stop.is_set():
            with self._lock:
                linked = self._transport is not None and self._transport.is_open

            if not linked:
                if not self.auto_var.get():
                    self._set_status("Disconnected (auto-reconnect off)")
                    time.sleep(0.5)
                    continue
                if not self._connect():
                    time.sleep(RECONNECT_S)
                    continue

            try:
                payload = collect_metrics(self._gpu, self._host_name)
                data = encode_metrics(payload)
                with self._lock:
                    transport = self._transport
                if transport is None:
                    continue
                transport.send(data)
                self._set_metrics(payload)
                self._set_status("Linked — streaming metrics")
                time.sleep(INTERVAL_S)
            except Exception as exc:  # noqa: BLE001
                self._set_status(f"Link lost: {exc}")
                with self._lock:
                    if self._transport is not None:
                        self._transport.close()
                        self._transport = None
                time.sleep(RECONNECT_S)

    def _on_close(self) -> None:
        self._stop.set()
        with self._lock:
            if self._transport is not None:
                self._transport.close()
                self._transport = None
        self._gpu.close()
        self.destroy()


def main() -> int:
    app = MonitorApp()
    app.mainloop()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
