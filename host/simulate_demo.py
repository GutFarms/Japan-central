#!/usr/bin/env python3
"""Emit demo metric packets (no sensors) for UI bring-up."""

from __future__ import annotations

import argparse
import json
import math
import socket
import time


def main() -> int:
    parser = argparse.ArgumentParser(description="Send synthetic metrics to ESP32-CYD")
    parser.add_argument("--host", required=True, help="CYD IP address")
    parser.add_argument("--port", type=int, default=4210)
    parser.add_argument("--interval", type=float, default=0.5)
    args = parser.parse_args()

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    t0 = time.time()
    print(f"Demo stream → {args.host}:{args.port}")
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
            sock.sendto(json.dumps(payload, separators=(",", ":")).encode(), (args.host, args.port))
            print(
                f"cpu={payload['cpu']:5.1f}% gpu={payload['gpu']:5.1f}%",
                end="\r",
                flush=True,
            )
            time.sleep(max(0.1, args.interval))
    except KeyboardInterrupt:
        print("\nStopped.")
    finally:
        sock.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
