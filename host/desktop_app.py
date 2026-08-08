#!/usr/bin/env python3
"""CYD Monitor desktop app — smooth dials, settings, tray, sequenced link."""

from __future__ import annotations

import math
import socket
import threading
import time
import tkinter as tk
from tkinter import ttk
from typing import Any, Optional

import psutil

from cyd_core import (
    GpuReader,
    LinkQuality,
    MetricsStream,
    SerialTransport,
    UdpTransport,
    collect_metrics,
    list_serial_port_infos,
    load_settings,
    pick_best_port,
    save_settings,
)

APP_TITLE = "CYD Monitor"
RECONNECT_S = 2.0

try:
    import pystray
    from PIL import Image, ImageDraw

    _HAS_TRAY = True
except Exception:  # noqa: BLE001
    _HAS_TRAY = False


def _tray_image() -> "Image.Image":
    img = Image.new("RGBA", (64, 64), (0, 12, 40, 255))
    draw = ImageDraw.Draw(img)
    draw.ellipse((6, 6, 58, 58), fill=(60, 140, 255, 255))
    draw.ellipse((20, 20, 44, 44), fill=(200, 230, 255, 255))
    return img


class Speedometer(tk.Canvas):
    """Speedometer with eased needle motion."""

    def __init__(self, master: tk.Misc, title: str, **kwargs: Any) -> None:
        super().__init__(master, highlightthickness=0, bg="#071833", **kwargs)
        self.title = title
        self.target = 0.0
        self.shown = 0.0
        self.sub = ""
        self._animating = False
        self.bind("<Configure>", lambda _e: self.redraw())

    def set_value(self, value: float, sub: str = "") -> None:
        self.target = max(0.0, min(100.0, float(value)))
        self.sub = sub
        if not self._animating:
            self._animating = True
            self._tick()

    def _tick(self) -> None:
        self.shown += (self.target - self.shown) * 0.22
        if abs(self.target - self.shown) < 0.15:
            self.shown = self.target
            self._animating = False
        self.redraw()
        if self._animating:
            self.after(33, self._tick)

    def redraw(self) -> None:
        self.delete("all")
        w = max(self.winfo_width(), 10)
        h = max(self.winfo_height(), 10)
        cx, cy = w / 2, h / 2 + 10
        r = min(w, h) * 0.40

        # Layered dark/light blue face
        self.create_oval(cx - r - 10, cy - r - 10, cx + r + 10, cy + r + 10, fill="#0A2048", outline="#1E4F9E", width=1)
        self.create_oval(cx - r - 4, cy - r - 4, cx + r + 4, cy + r + 4, fill="#153A7A", outline="#6EB6FF", width=2)
        self.create_oval(cx - r, cy - r, cx + r, cy + r, fill="#071833", outline="#9FD2FF", width=2)

        for i in range(0, 101, 5):
            theta = math.radians(225.0 - 270.0 * (i / 100.0))
            outer = r - 2
            inner = r - (12 if i % 10 == 0 else 6)
            x0 = cx + inner * math.cos(theta)
            y0 = cy - inner * math.sin(theta)
            x1 = cx + outer * math.cos(theta)
            y1 = cy - outer * math.sin(theta)
            color = "#FF8A8A" if i >= 90 else "#FFD27A" if i >= 75 else "#9FD2FF"
            self.create_line(x0, y0, x1, y1, fill=color, width=2 if i % 10 == 0 else 1)

        prev = None
        for i in range(0, 101, 2):
            theta = math.radians(225.0 - 270.0 * (i / 100.0))
            x = cx + (r - 16) * math.cos(theta)
            y = cy - (r - 16) * math.sin(theta)
            if prev is not None:
                c = "#FF8A8A" if i >= 90 else "#FFD27A" if i >= 75 else "#3D8CFF"
                self.create_line(prev[0], prev[1], x, y, fill=c, width=5, capstyle=tk.ROUND)
            prev = (x, y)

        # Value arc overlay to current shown value
        prev = None
        steps = max(2, int(self.shown / 2))
        for i in range(0, steps + 1):
            pct = self.shown * (i / steps) if steps else 0
            theta = math.radians(225.0 - 270.0 * (pct / 100.0))
            x = cx + (r - 16) * math.cos(theta)
            y = cy - (r - 16) * math.sin(theta)
            if prev is not None:
                self.create_line(prev[0], prev[1], x, y, fill="#B8E0FF", width=3, capstyle=tk.ROUND)
            prev = (x, y)

        theta = math.radians(225.0 - 270.0 * (self.shown / 100.0))
        nx = cx + (r - 24) * math.cos(theta)
        ny = cy - (r - 24) * math.sin(theta)
        self.create_line(cx, cy, nx, ny, fill="#F2F7FF", width=3, arrow=tk.LAST)
        self.create_oval(cx - 6, cy - 6, cx + 6, cy + 6, fill="#5CB8FF", outline="#E8F1FF")

        self.create_text(cx, cy - r - 4, text=self.title, fill="#9FD2FF", font=("Segoe UI", 11, "bold"))
        self.create_text(cx, cy + 20, text=f"{self.shown:.0f}%", fill="#F2F7FF", font=("Segoe UI", 16, "bold"))
        if self.sub:
            self.create_text(cx, cy + 40, text=self.sub, fill="#8FB4E8", font=("Segoe UI", 8))


class MonitorApp(tk.Tk):
    def __init__(self) -> None:
        super().__init__()
        self.settings = load_settings()
        self.title(APP_TITLE)
        self.geometry("900x680")
        self.minsize(740, 540)
        self.configure(bg="#071833")

        self._stop = threading.Event()
        self._worker: Optional[threading.Thread] = None
        self._transport: Optional[SerialTransport] = None
        self._udp: Optional[UdpTransport] = None
        self._gpu = GpuReader(index=int(self.settings.get("gpu_index", 0)))
        self._stream = MetricsStream(full_every=8)
        self._link = LinkQuality()
        self._pending: dict[int, float] = {}
        self._lock = threading.Lock()
        self._tray_icon = None

        host = (self.settings.get("host_name") or socket.gethostname())[:23]
        self.host_name = host

        self.status_var = tk.StringVar(value="Starting…")
        self.port_var = tk.StringVar(value="—")
        self.link_var = tk.StringVar(value="Link: —")
        self.extra_var = tk.StringVar(value="")
        self.auto_var = tk.BooleanVar(value=bool(self.settings.get("auto_reconnect", True)))
        self.start_min_var = tk.BooleanVar(value=bool(self.settings.get("start_minimized", False)))
        self.close_tray_var = tk.BooleanVar(value=bool(self.settings.get("close_to_tray", True)))
        self.interval_var = tk.DoubleVar(value=float(self.settings.get("interval", 0.5)))
        self.baud_var = tk.IntVar(value=int(self.settings.get("baud", 115200)))
        self.host_var = tk.StringVar(value=self.host_name)
        self.preferred_port_var = tk.StringVar(value=str(self.settings.get("preferred_port", "") or "auto"))
        self.udp_host_var = tk.StringVar(value=str(self.settings.get("udp_host", "")))
        self.udp_port_var = tk.IntVar(value=int(self.settings.get("udp_port", 4210)))

        self._build_style()
        self._build_ui()
        self.protocol("WM_DELETE_WINDOW", self._on_close_request)

        psutil.cpu_percent(interval=None)
        self.after(200, self._start_worker)
        if self.start_min_var.get():
            self.after(300, self._minimize_to_background)

    def _build_style(self) -> None:
        style = ttk.Style(self)
        try:
            style.theme_use("clam")
        except tk.TclError:
            pass
        bg, card, accent, text, muted = "#071833", "#12326B", "#3D8CFF", "#F2F7FF", "#9FD2FF"
        style.configure("TNotebook", background=bg, borderwidth=0)
        style.configure("TNotebook.Tab", background=card, foreground=text, padding=(14, 6))
        style.map("TNotebook.Tab", background=[("selected", accent)])
        style.configure("Root.TFrame", background=bg)
        style.configure("Card.TFrame", background=card)
        style.configure("Title.TLabel", background=bg, foreground="#6EB6FF", font=("Segoe UI", 18, "bold"))
        style.configure("Body.TLabel", background=card, foreground=text, font=("Segoe UI", 10))
        style.configure("Muted.TLabel", background=card, foreground=muted, font=("Segoe UI", 9))
        style.configure("Status.TLabel", background=bg, foreground="#7DFFB2", font=("Segoe UI", 11, "bold"))
        style.configure("Accent.TButton", background=accent, foreground="#FFFFFF", font=("Segoe UI", 10, "bold"), padding=8)
        style.map("Accent.TButton", background=[("active", "#5CA0FF")])
        style.configure("TCheckbutton", background=card, foreground=text)
        style.configure("TEntry", fieldbackground="#071833", foreground=text)
        style.configure("TCombobox", fieldbackground="#071833", foreground=text)

    def _build_ui(self) -> None:
        root = ttk.Frame(self, style="Root.TFrame", padding=12)
        root.pack(fill=tk.BOTH, expand=True)

        top = ttk.Frame(root, style="Root.TFrame")
        top.pack(fill=tk.X)
        ttk.Label(top, text="CYD Monitor", style="Title.TLabel").pack(side=tk.LEFT)
        ttk.Label(top, textvariable=self.status_var, style="Status.TLabel").pack(side=tk.RIGHT)

        nb = ttk.Notebook(root)
        nb.pack(fill=tk.BOTH, expand=True, pady=(10, 0))
        monitor = ttk.Frame(nb, style="Root.TFrame", padding=8)
        settings = ttk.Frame(nb, style="Card.TFrame", padding=16)
        nb.add(monitor, text="Monitor")
        nb.add(settings, text="Settings")

        dials = ttk.Frame(monitor, style="Root.TFrame")
        dials.pack(fill=tk.BOTH, expand=True)
        for r in range(2):
            dials.rowconfigure(r, weight=1)
        for c in range(2):
            dials.columnconfigure(c, weight=1)

        self.dial_cpu = Speedometer(dials, "CPU", width=300, height=230)
        self.dial_gpu = Speedometer(dials, "GPU", width=300, height=230)
        self.dial_ram = Speedometer(dials, "RAM", width=300, height=230)
        self.dial_vram = Speedometer(dials, "VRAM", width=300, height=230)
        self.dial_cpu.grid(row=0, column=0, sticky="nsew", padx=6, pady=6)
        self.dial_gpu.grid(row=0, column=1, sticky="nsew", padx=6, pady=6)
        self.dial_ram.grid(row=1, column=0, sticky="nsew", padx=6, pady=6)
        self.dial_vram.grid(row=1, column=1, sticky="nsew", padx=6, pady=6)

        info = ttk.Frame(monitor, style="Card.TFrame", padding=10)
        info.pack(fill=tk.X, pady=(8, 0))
        ttk.Label(info, textvariable=self.port_var, style="Muted.TLabel").pack(anchor=tk.W)
        ttk.Label(info, textvariable=self.link_var, style="Body.TLabel").pack(anchor=tk.W, pady=(2, 0))
        ttk.Label(info, textvariable=self.extra_var, style="Body.TLabel").pack(anchor=tk.W, pady=(4, 0))

        btns = ttk.Frame(monitor, style="Root.TFrame")
        btns.pack(fill=tk.X, pady=(8, 0))
        ttk.Button(btns, text="Minimize to tray", style="Accent.TButton", command=self._minimize_to_background).pack(
            side=tk.LEFT
        )
        ttk.Button(btns, text="Rescan USB", style="Accent.TButton", command=self._rescan).pack(side=tk.RIGHT)
        self._build_settings(settings)

    def _build_settings(self, parent: ttk.Frame) -> None:
        def row(label: str, widget: tk.Widget, r: int) -> None:
            ttk.Label(parent, text=label, style="Body.TLabel").grid(row=r, column=0, sticky="w", pady=6)
            widget.grid(row=r, column=1, sticky="ew", pady=6, padx=(12, 0))

        parent.columnconfigure(1, weight=1)
        ports = ["auto"] + [p.device for p in list_serial_port_infos()]
        self.port_combo = ttk.Combobox(parent, textvariable=self.preferred_port_var, values=ports, width=36)
        row("USB port", self.port_combo, 0)
        row("Baud", ttk.Entry(parent, textvariable=self.baud_var, width=12), 1)
        row("Interval (s)", ttk.Entry(parent, textvariable=self.interval_var, width=12), 2)
        row("Host label", ttk.Entry(parent, textvariable=self.host_var, width=24), 3)
        row("UDP host (optional)", ttk.Entry(parent, textvariable=self.udp_host_var, width=24), 4)
        row("UDP port", ttk.Entry(parent, textvariable=self.udp_port_var, width=12), 5)
        ttk.Checkbutton(parent, text="Auto-reconnect USB", variable=self.auto_var).grid(
            row=6, column=0, columnspan=2, sticky="w", pady=4
        )
        ttk.Checkbutton(parent, text="Start minimized to tray", variable=self.start_min_var).grid(
            row=7, column=0, columnspan=2, sticky="w", pady=4
        )
        ttk.Checkbutton(parent, text="Close button hides to tray (background)", variable=self.close_tray_var).grid(
            row=8, column=0, columnspan=2, sticky="w", pady=4
        )
        ttk.Button(parent, text="Save settings", style="Accent.TButton", command=self._save_settings).grid(
            row=9, column=0, columnspan=2, sticky="e", pady=(16, 0)
        )
        ttk.Label(
            parent,
            text="Uses sequenced packets + ACK/RTT. Delta updates keep the USB link light; full snapshots every few frames.",
            style="Muted.TLabel",
            wraplength=560,
        ).grid(row=10, column=0, columnspan=2, sticky="w", pady=(18, 0))

    def _save_settings(self) -> None:
        self.host_name = (self.host_var.get() or socket.gethostname())[:23]
        self.settings = {
            "interval": float(self.interval_var.get()),
            "baud": int(self.baud_var.get()),
            "auto_reconnect": bool(self.auto_var.get()),
            "start_minimized": bool(self.start_min_var.get()),
            "close_to_tray": bool(self.close_tray_var.get()),
            "host_name": self.host_name,
            "preferred_port": self.preferred_port_var.get(),
            "udp_host": self.udp_host_var.get().strip(),
            "udp_port": int(self.udp_port_var.get()),
            "gpu_index": int(self.settings.get("gpu_index", 0)),
        }
        save_settings(self.settings)
        self.status_var.set("Settings saved")
        self._rescan()

    def _set_status(self, text: str) -> None:
        self.after(0, lambda: self.status_var.set(text))

    def _set_port(self, text: str) -> None:
        self.after(0, lambda: self.port_var.set(text))

    def _set_link(self, text: str) -> None:
        self.after(0, lambda: self.link_var.set(text))

    def _set_metrics(self, payload: dict[str, Any]) -> None:
        def apply() -> None:
            self.dial_cpu.set_value(
                payload.get("cpu", 0),
                f"{payload.get('cpu_temp', 0):.0f}°C  {payload.get('cpu_mhz', 0):.0f} MHz",
            )
            self.dial_gpu.set_value(payload.get("gpu", 0), f"{payload.get('gpu_temp', 0):.0f}°C")
            self.dial_ram.set_value(
                payload.get("ram", 0),
                f"{payload.get('ram_used_gb', 0):.1f}/{payload.get('ram_total_gb', 0):.1f} GB",
            )
            self.dial_vram.set_value(payload.get("vram", 0), f"{payload.get('vram_used_mb', 0):.0f} MB")
            self.extra_var.set(
                f"Disk {payload.get('disk', 0):.0f}%   Swap {payload.get('swap', 0):.0f}%   "
                f"Net ↓ {payload.get('net_down', 0):.2f} / ↑ {payload.get('net_up', 0):.2f} Mbps   "
                f"Up {payload.get('uptime_min', 0)} min   "
                f"{self._gpu.name or 'No NVIDIA GPU'}"
            )

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
            if self._udp is not None:
                self._udp.close()
                self._udp = None
            self._stream = MetricsStream(full_every=8)
            self._link = LinkQuality()
            self._pending.clear()
        self._set_status("Rescanning…")
        self._set_port("—")
        self._set_link("Link: —")

    def _preferred(self) -> Optional[str]:
        val = (self.preferred_port_var.get() or "").strip()
        if not val or val.lower() == "auto":
            return None
        return val

    def _connect(self) -> bool:
        best = pick_best_port(self._preferred())
        if best is None:
            self._set_status("Waiting for ESP32-CYD USB…")
            self._set_port("No serial ports found")
            return False
        try:
            transport = SerialTransport(best.device, baud=int(self.baud_var.get()), settle_s=1.0)
        except Exception as exc:  # noqa: BLE001
            self._set_status(f"Open failed: {exc}")
            self._set_port(best.label)
            return False
        with self._lock:
            self._transport = transport
            udp_host = self.udp_host_var.get().strip()
            self._udp = UdpTransport(udp_host, int(self.udp_port_var.get())) if udp_host else None
            self._stream = MetricsStream(full_every=8)
            self._link = LinkQuality()
            self._pending.clear()
        self._set_status("Linked — streaming")
        self._set_port(best.label)
        return True

    def _handle_acks(self, acks: list[dict[str, Any]]) -> None:
        now = time.time()
        for ack in acks:
            seq = int(ack.get("seq") or 0)
            self._link.acks += 1
            self._link.last_ack_seq = seq
            sent_at = self._pending.pop(seq, None)
            if sent_at is not None:
                self._link.rtt_ms = (now - sent_at) * 1000.0

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
                payload = collect_metrics(self._gpu, self.host_name)
                with self._lock:
                    data = self._stream.encode(payload)
                    seq = self._stream.seq
                    transport = self._transport
                    udp = self._udp
                    self._pending[seq] = time.time()
                    # Bound pending map
                    if len(self._pending) > 40:
                        for old in sorted(self._pending.keys())[:-20]:
                            self._pending.pop(old, None)
                if transport is not None:
                    transport.send(data)
                    self._handle_acks(transport.poll_acks())
                if udp is not None:
                    udp.send(data)
                    self._handle_acks(udp.poll_acks())
                self._link.sent += 1
                self._set_metrics(payload)
                self._set_status("Linked — streaming")
                self._set_link(
                    f"Link: seq {self._link.last_ack_seq}   RTT {self._link.rtt_ms:.0f} ms   "
                    f"ACK {self._link.acks}/{self._link.sent}"
                )
                time.sleep(max(0.1, float(self.interval_var.get())))
            except Exception as exc:  # noqa: BLE001
                self._set_status(f"Link lost: {exc}")
                with self._lock:
                    if self._transport is not None:
                        self._transport.close()
                        self._transport = None
                time.sleep(RECONNECT_S)

    def _minimize_to_background(self) -> None:
        if _HAS_TRAY:
            self.withdraw()
            self._ensure_tray()
            self._set_status("Running in tray")
        else:
            self.iconify()
            self._set_status("Minimized (background)")

    def _ensure_tray(self) -> None:
        if not _HAS_TRAY or self._tray_icon is not None:
            return

        def show(_icon: Any = None, _item: Any = None) -> None:
            self.after(0, self._restore_from_tray)

        def quit_app(_icon: Any = None, _item: Any = None) -> None:
            self.after(0, self._quit_fully)

        menu = pystray.Menu(
            pystray.MenuItem("Show CYD Monitor", show, default=True),
            pystray.MenuItem("Quit", quit_app),
        )
        icon = pystray.Icon("cyd_monitor", _tray_image(), APP_TITLE, menu)
        self._tray_icon = icon
        threading.Thread(target=icon.run, name="cyd-tray", daemon=True).start()

    def _restore_from_tray(self) -> None:
        self.deiconify()
        self.lift()
        self.focus_force()

    def _on_close_request(self) -> None:
        if self.close_tray_var.get():
            self._minimize_to_background()
            return
        self._quit_fully()

    def _quit_fully(self) -> None:
        self._stop.set()
        with self._lock:
            if self._transport is not None:
                self._transport.close()
                self._transport = None
            if self._udp is not None:
                self._udp.close()
                self._udp = None
        if self._tray_icon is not None:
            try:
                self._tray_icon.stop()
            except Exception:  # noqa: BLE001
                pass
            self._tray_icon = None
        self._gpu.close()
        self.destroy()


def main() -> int:
    app = MonitorApp()
    app.mainloop()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
