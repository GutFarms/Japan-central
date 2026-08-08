#!/usr/bin/env python3
"""Emit demo metric packets (no sensors) for UI bring-up."""

from __future__ import annotations

import argparse
import json
import math
import socket
import sys
import time


def main() -> int:
    parser = argparse.ArgumentParser(description="Send synthetic metrics to ESP32-CYD")
    parser.add_argument("--serial", metavar="PORT", help="USB serial device (or 'auto')")
    parser.add_argument("--baud", type=int, default=115200)
    parser.add_argument("--host", help="CYD IP address for UDP")
    parser.add_argument("--port", type=int, default=4210)
    parser.add_argument("--interval", type=float, default=0.5)
    args = parser.parse_args()

    if not args.serial and not args.host:
        parser.error("Provide --serial PORT and/or --host IP")

    ser = None
    sock = None

    if args.serial:
        try:
            import serial
            from serial.tools import list_ports
        except ImportError as exc:
            raise SystemExit("pyserial required: pip install pyserial") from exc

        port = args.serial
        if port.lower() == "auto":
            found = [p.device for p in list_ports.comports()]
            if not found:
                print("No serial ports found", file=sys.stderr)
                return 1
            port = found[0]
            print(f"Auto-selected serial port: {port}")
        ser = serial.Serial(port=port, baudrate=args.baud, timeout=0.2)
        time.sleep(1.5)
        ser.reset_input_buffer()
        print(f"Demo stream → USB {port} @{args.baud}")

    if args.host:
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        print(f"Demo stream → UDP {args.host}:{args.port}")

    t0 = time.time()
    try:
        while True:
            t = time.time() - t0
            payload = {
                "v": 1,
                "cpu": round(55 + 35 * math.sin(t / 3.0), 1),
                "cpu_temp": round(58 + 10 * math.sin(t / 5.0), 1),
                "ram": round(48 + 12 * math.sin(t / 7.0), 1),
                "gpu": round(40 + 45 * math.sin(t / 2.2), 1),
                "gpu_temp": round(62 + 14 * math.sin(t / 4.0), 1),
                "vram": round(35 + 20 * math.sin(t / 6.0), 1),
                "fps": int(60 + 60 * (0.5 + 0.5 * math.sin(t / 1.5))),
                "host": "DEMO",
            }
            data = json.dumps(payload, separators=(",", ":")).encode("utf-8")
            if ser is not None:
                ser.write(data + b"\n")
                ser.flush()
            if sock is not None:
                sock.sendto(data, (args.host, args.port))
            print(
                f"cpu={payload['cpu']:5.1f}% gpu={payload['gpu']:5.1f}%",
                end="\r",
                flush=True,
            )
            time.sleep(max(0.1, args.interval))
    except KeyboardInterrupt:
        print("\nStopped.")
    finally:
        if ser is not None:
            ser.close()
        if sock is not None:
            sock.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
