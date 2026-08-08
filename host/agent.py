#!/usr/bin/env python3
"""CLI metrics agent for the ESP32-CYD monitor."""

from __future__ import annotations

import argparse
import socket
import sys
import time

import psutil

from cyd_core import (
    GpuReader,
    SerialTransport,
    UdpTransport,
    collect_metrics,
    encode_metrics,
    list_serial_ports,
    pick_best_port,
)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Send PC/GPU metrics to ESP32-CYD over USB serial and/or UDP"
    )
    parser.add_argument(
        "--serial",
        metavar="PORT",
        help="USB serial device (e.g. COM3, /dev/ttyUSB0). Use 'auto' to pick best CYD port.",
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

    transports = []

    if args.serial:
        port = args.serial
        if port.lower() == "auto":
            best = pick_best_port()
            if best is None:
                print("No serial ports found for --serial auto", file=sys.stderr)
                return 1
            port = best.device
            print(f"Auto-selected serial port: {best.label}")
        transports.append(SerialTransport(port, baud=args.baud))

    if args.host:
        transports.append(UdpTransport(args.host, args.port))

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
            data = encode_metrics(payload)
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
