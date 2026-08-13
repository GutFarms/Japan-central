#!/usr/bin/env python3
"""Verify CYD `cmp` answers on two independent UART endpoints (not just one COM).

Simulates two boards with distinct MACs on a pair of PTYs. Holds the first port
open (like a linked COM6) while probing the second — the failure mode users hit
when only one USB board is found.

No pyserial required — stdlib pty + os.read/write.
"""
from __future__ import annotations

import os
import select
import sys
import threading
import time


def fake_cyd(slave_fd: int, mac: str, fw: str = "0.8.50-sha256") -> None:
    buf = b""
    while True:
        r, _, _ = select.select([slave_fd], [], [], 0.2)
        if not r:
            continue
        try:
            chunk = os.read(slave_fd, 256)
        except OSError:
            return
        if not chunk:
            return
        buf += chunk
        while b"\n" in buf or b"\r" in buf:
            for sep in (b"\n", b"\r"):
                if sep in buf:
                    line, buf = buf.split(sep, 1)
                    break
            else:
                break
            text = line.decode("utf-8", "ignore").strip().lower()
            if not text:
                continue
            if text.startswith("cmp ping") or text == "ping":
                reply = f"CMP ok cmp mac={mac}\r\n".encode()
                os.write(slave_fd, reply)
            elif text.startswith("cmp config"):
                body = (
                    '{"cpu_mhz":240,"hash_focus":true,'
                    f'"fw":"{fw}","mode":"usb-wifi-sha256","mac":"{mac}"}}\r\n'
                )
                os.write(slave_fd, f"CMPCONFIG {body}".encode())


def drain(fd: int, timeout: float = 0.05) -> bytes:
    out = b""
    end = time.time() + timeout
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], max(0.0, end - time.time()))
        if not r:
            break
        try:
            chunk = os.read(fd, 512)
        except OSError:
            break
        if not chunk:
            break
        out += chunk
    return out


def wait_pong(fd: int, wait_s: float = 2.0) -> str | None:
    os.write(fd, b"\r\ncmp ping\r\n")
    buf = b""
    end = time.time() + wait_s
    while time.time() < end:
        buf += drain(fd, 0.05)
        for line in buf.splitlines():
            t = line.decode("utf-8", "ignore").strip()
            if t.startswith("CMP ok") or t.upper() == "CMPACK PING":
                return t
        time.sleep(0.02)
    return None


def open_pty() -> tuple[int, int, str]:
    master, slave = os.openpty()
    name = os.ttyname(slave)
    # Non-blocking-ish reads via select; keep blocking fds.
    return master, slave, name


def main() -> int:
    m1, s1, name1 = open_pty()
    m2, s2, name2 = open_pty()
    mac1 = "aa:bb:cc:dd:ee:01"
    mac2 = "aa:bb:cc:dd:ee:02"

    t1 = threading.Thread(target=fake_cyd, args=(s1, mac1), daemon=True)
    t2 = threading.Thread(target=fake_cyd, args=(s2, mac2), daemon=True)
    t1.start()
    t2.start()
    time.sleep(0.1)

    print(f"fake board A on {name1} mac={mac1}")
    print(f"fake board B on {name2} mac={mac2}")

    # 1) Both answer while neither is "linked".
    p1 = wait_pong(m1)
    p2 = wait_pong(m2)
    if not p1 or mac1 not in p1:
        print(f"FAIL: board A no pong ({p1!r})", file=sys.stderr)
        return 1
    if not p2 or mac2 not in p2:
        print(f"FAIL: board B no pong ({p2!r})", file=sys.stderr)
        return 1
    print(f"ok · both answer independently · A={p1} · B={p2}")

    # 2) Hold A open (linked COM6), still talk to B (second USB).
    # Keep reading A so the fake thread does not block on a full PTY buffer.
    stop = threading.Event()
    a_lock = threading.Lock()

    def hold_a() -> None:
        while not stop.is_set():
            with a_lock:
                drain(m1, 0.05)
            time.sleep(0.02)

    holder = threading.Thread(target=hold_a, daemon=True)
    holder.start()
    time.sleep(0.05)

    p2b = wait_pong(m2)
    if not p2b or mac2 not in p2b:
        print(f"FAIL: board B silent while A held open ({p2b!r})", file=sys.stderr)
        return 1
    print(f"ok · board B answers while A held open · {p2b}")

    # 3) Re-ping A under hold — still alive (serialize against the drain thread).
    with a_lock:
        p1b = wait_pong(m1)
    if not p1b or mac1 not in p1b:
        print(f"FAIL: board A lost while held ({p1b!r})", file=sys.stderr)
        return 1
    print(f"ok · board A still answers under hold · {p1b}")

    stop.set()
    holder.join(timeout=1)
    for fd in (m1, m2, s1, s2):
        try:
            os.close(fd)
        except OSError:
            pass

    print("PASS — multi-UART cmp ping works outside a single COM")
    return 0


if __name__ == "__main__":
    sys.exit(main())
