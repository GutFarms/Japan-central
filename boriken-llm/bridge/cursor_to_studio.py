#!/usr/bin/env python3
"""Cursor / CLI side of the Borikén Studio bridge — enqueue commands & read status."""

from __future__ import annotations

import argparse
import json
import sys
import time
import uuid
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MAILBOX = ROOT / "bridge" / "mailbox"
INBOX = MAILBOX / "inbox.jsonl"
OUTBOX = MAILBOX / "outbox.jsonl"
STATUS = MAILBOX / "status.json"

ACTIONS = {
    "ping",
    "sync_assets",
    "build_debug",
    "build_release",
    "adb_devices",
    "adb_install",
    "git_pull",
    "status",
}


def _now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def ensure_mailbox() -> None:
    MAILBOX.mkdir(parents=True, exist_ok=True)
    for p in (INBOX, OUTBOX):
        if not p.exists():
            p.write_text("", encoding="utf-8")
    if not STATUS.exists():
        STATUS.write_text(
            json.dumps(
                {
                    "ok": True,
                    "message": "mailbox ready — start studio_watcher.py on the Studio PC",
                    "ts": _now(),
                    "watcher": "offline",
                },
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )


def read_status() -> dict:
    ensure_mailbox()
    try:
        return json.loads(STATUS.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {"ok": False, "message": "status.json corrupt"}


def enqueue(action: str, source: str = "cursor", note: str = "", args: dict | None = None) -> dict:
    if action not in ACTIONS or action == "status":
        raise SystemExit(f"unknown or non-enqueue action: {action}")
    ensure_mailbox()
    cmd = {
        "id": str(uuid.uuid4()),
        "ts": _now(),
        "source": source,
        "action": action,
        "args": args or {},
        "note": note,
    }
    with INBOX.open("a", encoding="utf-8") as f:
        f.write(json.dumps(cmd, ensure_ascii=False) + "\n")
    print(json.dumps({"enqueued": True, **cmd}, indent=2))
    return cmd


def wait_for(cmd_id: str, timeout: float = 120.0) -> dict:
    deadline = time.time() + timeout
    while time.time() < deadline:
        if OUTBOX.exists():
            for line in OUTBOX.read_text(encoding="utf-8").splitlines():
                if not line.strip():
                    continue
                try:
                    row = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if row.get("id") == cmd_id:
                    print(json.dumps(row, indent=2))
                    return row
        time.sleep(0.5)
    raise SystemExit(f"timeout waiting for result id={cmd_id}")


def main() -> None:
    ap = argparse.ArgumentParser(description="Cursor → Android Studio bridge client")
    sub = ap.add_subparsers(dest="cmd", required=True)

    p_en = sub.add_parser("enqueue", help="queue an action for studio_watcher")
    p_en.add_argument("action", choices=sorted(ACTIONS - {"status"}))
    p_en.add_argument("--note", default="")
    p_en.add_argument("--source", default="cursor")
    p_en.add_argument("--wait", action="store_true", help="block until watcher reports")
    p_en.add_argument("--timeout", type=float, default=300.0)

    p_st = sub.add_parser("status", help="print mailbox status.json")
    p_st.add_argument("--watch", type=float, default=0, help="poll N seconds")

    p_ping = sub.add_parser("ping", help="enqueue ping")
    p_ping.add_argument("--wait", action="store_true")

    args = ap.parse_args()

    if args.cmd == "status":
        if args.watch > 0:
            end = time.time() + args.watch
            while time.time() < end:
                print(json.dumps(read_status(), indent=2))
                time.sleep(2)
        else:
            print(json.dumps(read_status(), indent=2))
        return

    if args.cmd == "ping":
        cmd = enqueue("ping", note="hello from cursor")
        if args.wait:
            wait_for(cmd["id"])
        return

    if args.cmd == "enqueue":
        cmd = enqueue(args.action, source=args.source, note=args.note)
        if args.wait:
            wait_for(cmd["id"], timeout=args.timeout)
        return


if __name__ == "__main__":
    main()
