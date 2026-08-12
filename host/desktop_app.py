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
# Prefer distinctive UI fonts; cascade falls back cleanly on Linux.
UI_FONT = "Trebuchet MS"
UI_FONT_BOLD = "Trebuchet MS"

try:
    import pystray
    from PIL import Image, ImageDraw

    _HAS_TRAY = True
except Exception:  # noqa: BLE001
    _HAS_TRAY = False


def _font(size: int, bold: bool = False) -> tuple:
    return (UI_FONT_BOLD, size, "bold") if bold else (UI_FONT, size)


def _tray_image() -> "Image.Image":
    img = Image.new("RGBA", (64, 64), (0, 6, 22, 255))
    draw = ImageDraw.Draw(img)
    draw.ellipse((4, 4, 60, 60), fill=(12, 40, 90, 255))
    draw.ellipse((10, 10, 54, 54), outline=(90, 160, 230, 255), width=3)
    draw.ellipse((22, 22, 42, 42), fill=(200, 230, 255, 255))
    return img


def _heat_color(pct: float) -> str:
    if pct >= 90:
        return "#E87878"
    if pct >= 75:
        return "#E0B85A"
    if pct >= 45:
        return "#5C9BE0"
    return "#3D7AC8"


def _temp_color(celsius: float) -> str:
    if celsius <= 0:
        return "#6A90B8"
    if celsius >= 90:
        return "#E87878"
    if celsius >= 75:
        return "#E0B85A"
    return "#7AD0F0"


class Speedometer(tk.Canvas):
    """Glowing speedometer with eased needle and scale marks."""

    def __init__(self, master: tk.Misc, title: str, **kwargs: Any) -> None:
        super().__init__(master, highlightthickness=0, bg="#020B1A", **kwargs)
        self.title = title
        self.target = 0.0
        self.shown = 0.0
        self.sub = ""
        self.temp_c = 0.0
        self._history: list[float] = []
        self._animating = False
        self.bind("<Configure>", lambda _e: self.redraw())

    def set_value(self, value: float, sub: str = "", temp_c: float = 0.0) -> None:
        self.target = max(0.0, min(100.0, float(value)))
        self.sub = sub
        self.temp_c = float(temp_c or 0.0)
        self._history.append(self.target)
        if len(self._history) > 48:
            self._history = self._history[-48:]
        if not self._animating:
            self._animating = True
            self._tick()

    def _tick(self) -> None:
        self.shown += (self.target - self.shown) * 0.24
        if abs(self.target - self.shown) < 0.12:
            self.shown = self.target
            self._animating = False
        self.redraw()
        if self._animating:
            self.after(33, self._tick)

    def redraw(self) -> None:
        self.delete("all")
        w = max(self.winfo_width(), 10)
        h = max(self.winfo_height(), 10)
        cx, cy = w / 2, h / 2 + 8
        r = min(w, h) * 0.38

        # Atmospheric backdrop wash
        self.create_oval(cx - r - 28, cy - r - 18, cx + r + 28, cy + r + 34, fill="#031226", outline="")
        self.create_oval(cx - r - 16, cy - r - 12, cx + r + 16, cy + r + 16, fill="#041830", outline="#0A2A55", width=1)
        self.create_oval(cx - r - 8, cy - r - 8, cx + r + 8, cy + r + 8, fill="#071F3F", outline="#1A4A88", width=2)
        self.create_oval(cx - r, cy - r, cx + r, cy + r, fill="#020B1A", outline="#8EC4F0", width=2)

        # Tick marks + labels
        for i in range(0, 101, 5):
            theta = math.radians(225.0 - 270.0 * (i / 100.0))
            major = i % 10 == 0
            outer = r - 2
            inner = r - (14 if major else 7)
            x0 = cx + inner * math.cos(theta)
            y0 = cy - inner * math.sin(theta)
            x1 = cx + outer * math.cos(theta)
            y1 = cy - outer * math.sin(theta)
            color = _heat_color(float(i))
            self.create_line(x0, y0, x1, y1, fill=color, width=3 if major else 1, capstyle=tk.ROUND)
            if i in (0, 50, 100):
                lx = cx + (r - 28) * math.cos(theta)
                ly = cy - (r - 28) * math.sin(theta)
                self.create_text(lx, ly, text=str(i), fill="#6A90B8", font=_font(8))

        # Track + value arc (dense segments for smooth look)
        prev = None
        for i in range(0, 101):
            theta = math.radians(225.0 - 270.0 * (i / 100.0))
            x = cx + (r - 18) * math.cos(theta)
            y = cy - (r - 18) * math.sin(theta)
            if prev is not None:
                self.create_line(prev[0], prev[1], x, y, fill="#12325C", width=8, capstyle=tk.ROUND)
            prev = (x, y)

        prev = None
        steps = max(2, int(self.shown))
        for i in range(0, steps + 1):
            pct = self.shown * (i / steps) if steps else 0
            theta = math.radians(225.0 - 270.0 * (pct / 100.0))
            x = cx + (r - 18) * math.cos(theta)
            y = cy - (r - 18) * math.sin(theta)
            if prev is not None:
                col = _heat_color(pct)
                self.create_line(prev[0], prev[1], x, y, fill=col, width=9, capstyle=tk.ROUND)
                self.create_line(prev[0], prev[1], x, y, fill="#C8E6FF", width=3, capstyle=tk.ROUND)
            prev = (x, y)

        # Needle
        theta = math.radians(225.0 - 270.0 * (self.shown / 100.0))
        nx = cx + (r - 26) * math.cos(theta)
        ny = cy - (r - 26) * math.sin(theta)
        self.create_line(cx, cy, nx, ny, fill="#F2F7FF", width=4, arrow=tk.LAST, capstyle=tk.ROUND)
        hub = _temp_color(self.temp_c) if self.temp_c > 0 else "#2A6AB8"
        self.create_oval(cx - 8, cy - 8, cx + 8, cy + 8, fill=hub, outline="#E8F0FA", width=2)
        self.create_oval(cx - 3, cy - 3, cx + 3, cy + 3, fill="#F2F7FF", outline="")

        # Sparkline history under value
        if len(self._history) > 2:
            sx0, sy0 = cx - r * 0.55, cy + r * 0.55
            sw, sh = r * 1.1, 16
            pts: list[float] = []
            for idx, val in enumerate(self._history):
                px = sx0 + sw * (idx / max(1, len(self._history) - 1))
                py = sy0 + sh - (sh * val / 100.0)
                pts.extend([px, py])
            self.create_line(*pts, fill="#3D7AC8", width=2, smooth=True)

        self.create_text(cx, cy - r - 6, text=self.title, fill="#9EC8F0", font=_font(12, bold=True))
        self.create_text(cx, cy + 18, text=f"{self.shown:.0f}%", fill="#F2F7FF", font=_font(20, bold=True))
        if self.sub:
            self.create_text(cx, cy + 40, text=self.sub, fill="#7AA8D8", font=_font(9))


class BarMeter(tk.Canvas):
    """Horizontal usage bar with used / free captions."""

    def __init__(self, master: tk.Misc, title: str, **kwargs: Any) -> None:
        super().__init__(master, highlightthickness=0, bg="#0A224E", height=78, **kwargs)
        self.title = title
        self.pct = 0.0
        self.line1 = ""
        self.line2 = ""
        self.bind("<Configure>", lambda _e: self.redraw())

    def set_value(self, pct: float, line1: str = "", line2: str = "") -> None:
        self.pct = max(0.0, min(100.0, float(pct)))
        self.line1 = line1
        self.line2 = line2
        self.redraw()

    def redraw(self) -> None:
        self.delete("all")
        w = max(self.winfo_width(), 10)
        h = max(self.winfo_height(), 10)
        self.create_rectangle(0, 0, w, h, fill="#0A224E", outline="")
        self.create_text(12, 14, anchor="w", text=self.title, fill="#9EC8F0", font=_font(11, bold=True))
        self.create_text(w - 12, 14, anchor="e", text=f"{self.pct:.0f}%", fill="#F2F7FF", font=_font(12, bold=True))
        bx, by, bh = 12, 34, 14
        bw = max(20, w - 24)
        self.create_rectangle(bx, by, bx + bw, by + bh, fill="#031226", outline="#1A4A88")
        fill_w = max(0, int(bw * self.pct / 100.0))
        if fill_w > 0:
            self.create_rectangle(bx, by, bx + fill_w, by + bh, fill=_heat_color(self.pct), outline="")
        self.create_text(12, h - 14, anchor="w", text=self.line1, fill="#C8DCF0", font=_font(9))
        self.create_text(w - 12, h - 14, anchor="e", text=self.line2, fill="#7AA8D8", font=_font(9))


class InfoCard(tk.Canvas):
    """Multi-line info card for temps / OC clocks."""

    def __init__(self, master: tk.Misc, title: str, **kwargs: Any) -> None:
        super().__init__(master, highlightthickness=0, bg="#0A224E", height=78, **kwargs)
        self.title = title
        self.hero = "—"
        self.hero_color = "#F2F7FF"
        self.lines: list[str] = []
        self.bind("<Configure>", lambda _e: self.redraw())

    def set_content(self, hero: str, lines: list[str] | None = None, hero_color: str = "#F2F7FF") -> None:
        self.hero = hero
        self.lines = lines or []
        self.hero_color = hero_color
        self.redraw()

    def redraw(self) -> None:
        self.delete("all")
        w = max(self.winfo_width(), 10)
        h = max(self.winfo_height(), 10)
        self.create_rectangle(0, 0, w, h, fill="#0A224E", outline="")
        self.create_rectangle(1, 1, w - 2, h - 2, outline="#1A4A88")
        self.create_text(12, 14, anchor="w", text=self.title, fill="#9EC8F0", font=_font(10, bold=True))
        self.create_text(12, 40, anchor="w", text=self.hero, fill=self.hero_color, font=_font(18, bold=True))
        y = 58
        for line in self.lines[:2]:
            self.create_text(12, y, anchor="w", text=line, fill="#7AA8D8", font=_font(8))
            y += 12


class MonitorApp(tk.Tk):
    def __init__(self) -> None:
        super().__init__()
        self.settings = load_settings()
        self.title(APP_TITLE)
        self.geometry("1040x760")
        self.minsize(860, 640)
        self.configure(bg="#020B1A")

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
        self.cyd_orient_var = tk.StringVar(value="Normal")
        self.cyd_bright_var = tk.IntVar(value=220)
        self.cyd_cfg_var = tk.StringVar(value="CYD: (not synced)")
        self._cmd_queue: list[tuple[str, dict[str, Any]]] = []

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
        bg, card, accent, text, muted = "#020B1A", "#0A224E", "#1E5AA8", "#E8F0FA", "#7AA8D8"
        style.configure("TNotebook", background=bg, borderwidth=0)
        style.configure("TNotebook.Tab", background=card, foreground=text, padding=(14, 6), font=(UI_FONT, 10, "bold"))
        style.map("TNotebook.Tab", background=[("selected", accent)])
        style.configure("Root.TFrame", background=bg)
        style.configure("Card.TFrame", background=card)
        style.configure("Title.TLabel", background=bg, foreground="#6AA8E8", font=(UI_FONT, 20, "bold"))
        style.configure("Body.TLabel", background=card, foreground=text, font=(UI_FONT, 10))
        style.configure("Muted.TLabel", background=card, foreground=muted, font=(UI_FONT, 9))
        style.configure("Status.TLabel", background=bg, foreground="#5AD89A", font=(UI_FONT, 11, "bold"))
        style.configure("Accent.TButton", background=accent, foreground="#FFFFFF", font=(UI_FONT, 10, "bold"), padding=8)
        style.map("Accent.TButton", background=[("active", "#2A6AB8")])
        style.configure("TCheckbutton", background=card, foreground=text)
        style.configure("TEntry", fieldbackground="#020B1A", foreground=text)
        style.configure("TCombobox", fieldbackground="#020B1A", foreground=text)

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
        dials.columnconfigure(0, weight=3)
        dials.columnconfigure(1, weight=2)
        dials.rowconfigure(0, weight=1)
        dials.rowconfigure(1, weight=1)

        self.dial_cpu = Speedometer(dials, "CPU", width=420, height=420)
        self.dial_gpu = Speedometer(dials, "GPU", width=280, height=200)
        self.dial_ram = Speedometer(dials, "RAM", width=280, height=200)
        self.dial_cpu.grid(row=0, column=0, rowspan=2, sticky="nsew", padx=(6, 4), pady=6)
        self.dial_gpu.grid(row=0, column=1, sticky="nsew", padx=(4, 6), pady=(6, 3))
        self.dial_ram.grid(row=1, column=1, sticky="nsew", padx=(4, 6), pady=(3, 6))

        stats = ttk.Frame(monitor, style="Root.TFrame")
        stats.pack(fill=tk.X, pady=(4, 0))
        for c in range(4):
            stats.columnconfigure(c, weight=1)

        self.card_temp = InfoCard(stats, "CPU TEMP", width=200)
        self.card_oc = InfoCard(stats, "OC / CLOCKS", width=240)
        self.card_disk = BarMeter(stats, "DISK", width=240)
        self.card_vram = BarMeter(stats, "VRAM", width=200)
        self.card_temp.grid(row=0, column=0, sticky="nsew", padx=(6, 3), pady=4)
        self.card_oc.grid(row=0, column=1, sticky="nsew", padx=3, pady=4)
        self.card_disk.grid(row=0, column=2, sticky="nsew", padx=3, pady=4)
        self.card_vram.grid(row=0, column=3, sticky="nsew", padx=(3, 6), pady=4)

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
        ttk.Button(btns, text="Flip CYD 180°", style="Accent.TButton", command=self._flip_cyd).pack(
            side=tk.LEFT, padx=(8, 0)
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

        ttk.Separator(parent, orient=tk.HORIZONTAL).grid(row=9, column=0, columnspan=2, sticky="ew", pady=12)
        ttk.Label(parent, text="CYD display (saved on device)", style="Body.TLabel").grid(
            row=10, column=0, columnspan=2, sticky="w"
        )
        ttk.Label(parent, textvariable=self.cyd_cfg_var, style="Muted.TLabel").grid(
            row=11, column=0, columnspan=2, sticky="w", pady=(2, 8)
        )

        orient = ttk.Combobox(
            parent,
            textvariable=self.cyd_orient_var,
            values=("Normal", "Flipped 180°"),
            state="readonly",
            width=18,
        )
        row("Screen orientation", orient, 12)

        bright_row = ttk.Frame(parent, style="Card.TFrame")
        bright = ttk.Scale(
            bright_row, from_=20, to=255, orient=tk.HORIZONTAL, variable=self.cyd_bright_var, command=self._on_bright_slide
        )
        bright.pack(side=tk.LEFT, fill=tk.X, expand=True)
        self.cyd_bright_label = ttk.Label(bright_row, text="86%", style="Muted.TLabel", width=5)
        self.cyd_bright_label.pack(side=tk.LEFT, padx=(8, 0))
        bright.bind("<ButtonRelease-1>", lambda _e: self._apply_cyd_settings())
        row("Brightness", bright_row, 13)
        self._on_bright_slide(self.cyd_bright_var.get())

        cyd_btns = ttk.Frame(parent, style="Card.TFrame")
        cyd_btns.grid(row=14, column=0, columnspan=2, sticky="ew", pady=(8, 0))
        ttk.Button(cyd_btns, text="Apply orientation + brightness", style="Accent.TButton", command=self._apply_cyd_settings).pack(
            side=tk.LEFT
        )
        ttk.Button(cyd_btns, text="Flip 180° now", style="Accent.TButton", command=self._flip_cyd).pack(
            side=tk.LEFT, padx=(8, 0)
        )
        ttk.Button(cyd_btns, text="Sync from CYD", style="Accent.TButton", command=self._request_cyd_config).pack(
            side=tk.LEFT, padx=(8, 0)
        )

        ttk.Button(parent, text="Save app settings", style="Accent.TButton", command=self._save_settings).grid(
            row=15, column=0, columnspan=2, sticky="e", pady=(16, 0)
        )
        ttk.Label(
            parent,
            text="Tip: release the brightness slider to push it to the CYD. Flip 180° is instant and stored in NVS.",
            style="Muted.TLabel",
            wraplength=560,
        ).grid(row=16, column=0, columnspan=2, sticky="w", pady=(12, 0))

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
        self.status_var.set("App settings saved")
        self._rescan()

    def _queue_cmd(self, cmd: str, **fields: Any) -> None:
        with self._lock:
            self._cmd_queue.append((cmd, fields))
        self._set_status(f"Queued CYD command: {cmd}")

    def _on_bright_slide(self, value: Any) -> None:
        try:
            level = int(float(value))
        except (TypeError, ValueError):
            level = int(self.cyd_bright_var.get())
        pct = max(0, min(100, int(round((level - 20) * 100 / (255 - 20)))))
        if hasattr(self, "cyd_bright_label"):
            self.cyd_bright_label.configure(text=f"{pct}%")

    def _flip_cyd(self) -> None:
        # Optimistic UI toggle so the control matches what the screen will do.
        flipped = self.cyd_orient_var.get().startswith("Flipped")
        self.cyd_orient_var.set("Normal" if flipped else "Flipped 180°")
        self._queue_cmd("flip")
        self._set_status("Flipping CYD screen 180°…")

    def _apply_cyd_settings(self) -> None:
        flip = self.cyd_orient_var.get().startswith("Flipped")
        bright = int(self.cyd_bright_var.get())
        self._queue_cmd(
            "set",
            flip=flip,
            rot=3 if flip else 1,
            bright=bright,
        )
        self._set_status(f"Sending CYD display settings (bright {bright})…")

    def _request_cyd_config(self) -> None:
        self._queue_cmd("get")
        self._set_status("Reading CYD display settings…")

    def _apply_device_cfg(self, cfg: dict[str, Any]) -> None:
        def apply() -> None:
            rot = int(cfg.get("rot", 1))
            bright = int(cfg.get("bright", 220))
            flip = bool(cfg.get("flip", rot == 3))
            self.cyd_orient_var.set("Flipped 180°" if flip or rot == 3 else "Normal")
            self.cyd_bright_var.set(bright)
            self._on_bright_slide(bright)
            orient = "Flipped 180°" if flip or rot == 3 else "Normal"
            pct = max(0, min(100, int(round((bright - 20) * 100 / (255 - 20)))))
            self.cyd_cfg_var.set(f"On CYD now: {orient} · brightness {pct}% ({bright}/255)")

        self.after(0, apply)

    def _set_status(self, text: str) -> None:
        self.after(0, lambda: self.status_var.set(text))

    def _set_port(self, text: str) -> None:
        self.after(0, lambda: self.port_var.set(text))

    def _set_link(self, text: str) -> None:
        self.after(0, lambda: self.link_var.set(text))

    def _set_metrics(self, payload: dict[str, Any]) -> None:
        def apply() -> None:
            cpu_temp = float(payload.get("cpu_temp", 0) or 0)
            gpu_temp = float(payload.get("gpu_temp", 0) or 0)
            cpu_mhz = float(payload.get("cpu_mhz", 0) or 0)
            cpu_max = float(payload.get("cpu_mhz_max", 0) or 0)
            cpu_min = float(payload.get("cpu_mhz_min", 0) or 0)
            boost = float(payload.get("cpu_boost_pct", 0) or 0)
            gpu_clk = float(payload.get("gpu_clock_mhz", 0) or 0)
            gpu_max = float(payload.get("gpu_clock_max_mhz", 0) or 0)
            gpu_mem = float(payload.get("gpu_mem_clock_mhz", 0) or 0)
            disk_pct = float(payload.get("disk", 0) or 0)
            disk_used = float(payload.get("disk_used_gb", 0) or 0)
            disk_total = float(payload.get("disk_total_gb", 0) or 0)
            disk_free = float(payload.get("disk_free_gb", 0) or 0)
            vram = float(payload.get("vram", 0) or 0)
            vram_mb = float(payload.get("vram_used_mb", 0) or 0)

            temp_txt = f"{cpu_temp:.0f}°C" if cpu_temp > 0 else "n/a"
            self.dial_cpu.set_value(
                payload.get("cpu", 0),
                f"{temp_txt}  ·  {cpu_mhz:.0f} MHz",
                temp_c=cpu_temp,
            )
            self.dial_gpu.set_value(
                payload.get("gpu", 0),
                f"{gpu_temp:.0f}°C" if gpu_temp > 0 else "GPU",
                temp_c=gpu_temp,
            )
            self.dial_ram.set_value(
                payload.get("ram", 0),
                f"{payload.get('ram_used_gb', 0):.1f}/{payload.get('ram_total_gb', 0):.1f} GB",
            )

            self.card_temp.set_content(
                f"{cpu_temp:.0f}°C" if cpu_temp > 0 else "—",
                [
                    f"GPU {gpu_temp:.0f}°C" if gpu_temp > 0 else "GPU temp n/a",
                    f"{int(payload.get('cpu_cores', 0) or 0)}C / {int(payload.get('cpu_threads', 0) or 0)}T",
                ],
                hero_color=_temp_color(cpu_temp),
            )

            oc_lines = [
                f"CPU {cpu_min:.0f}–{cpu_max:.0f} MHz" if cpu_max > 0 else "CPU clocks n/a",
            ]
            if gpu_clk > 0:
                oc_lines.append(f"GPU {gpu_clk:.0f}/{gpu_max:.0f} · MEM {gpu_mem:.0f} MHz")
            else:
                oc_lines.append(self._gpu.name or "No NVIDIA GPU")
            self.card_oc.set_content(
                f"{cpu_mhz:.0f} MHz" if cpu_mhz > 0 else "—",
                oc_lines,
                hero_color="#8EC4F0" if boost < 95 else "#E0B85A",
            )

            self.card_disk.set_value(
                disk_pct,
                f"{disk_used:.1f} / {disk_total:.1f} GB used",
                f"{disk_free:.1f} GB free",
            )
            self.card_vram.set_value(
                vram,
                f"{vram_mb:.0f} MB used",
                f"Swap {payload.get('swap', 0):.0f}%",
            )

            self.extra_var.set(
                f"Net ↓ {payload.get('net_down', 0):.2f} / ↑ {payload.get('net_up', 0):.2f} Mbps   "
                f"Boost {boost:.0f}% of max   "
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
            self._cmd_queue.append(("get", {}))
        self._set_status("Linked — streaming")
        self._set_port(best.label)
        return True

    def _handle_messages(self, messages: list[dict[str, Any]]) -> None:
        now = time.time()
        for msg in messages:
            if "cfg" in msg and isinstance(msg["cfg"], dict):
                self._apply_device_cfg(msg["cfg"])
                self._set_status("CYD settings updated")
            seq = int(msg.get("seq") or 0)
            if seq:
                self._link.acks += 1
                self._link.last_ack_seq = seq
                sent_at = self._pending.pop(seq, None)
                if sent_at is not None:
                    self._link.rtt_ms = (now - sent_at) * 1000.0

    def _flush_commands(self, transport: Optional[SerialTransport], udp: Optional[UdpTransport]) -> None:
        with self._lock:
            queued = list(self._cmd_queue)
            self._cmd_queue.clear()
        for cmd, fields in queued:
            if transport is not None:
                transport.send_command(cmd, **fields)
                self._handle_messages(transport.poll_messages())
            if udp is not None:
                udp.send_command(cmd, **fields)
                self._handle_messages(udp.poll_messages())

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
                with self._lock:
                    transport = self._transport
                    udp = self._udp
                self._flush_commands(transport, udp)

                payload = collect_metrics(self._gpu, self.host_name)
                with self._lock:
                    data = self._stream.encode(payload)
                    seq = self._stream.seq
                    self._pending[seq] = time.time()
                    if len(self._pending) > 40:
                        for old in sorted(self._pending.keys())[:-20]:
                            self._pending.pop(old, None)
                if transport is not None:
                    transport.send(data)
                    self._handle_messages(transport.poll_messages())
                if udp is not None:
                    udp.send(data)
                    self._handle_messages(udp.poll_messages())
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
