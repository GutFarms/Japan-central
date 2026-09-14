#!/usr/bin/env python3
"""
Local watcher for the Borikén Cursor ↔ Android Studio bridge.

Run on the machine that has Android Studio / Android SDK / adb:

    python3 bridge/studio_watcher.py --watch

It drains bridge/mailbox/inbox.jsonl and executes Gradle / ADB actions.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ANDROID = ROOT / "android"
WEBAPP = ROOT / "webapp" / "index.html"
ASSETS = ANDROID / "app" / "src" / "main" / "assets" / "www" / "index.html"
MAILBOX = ROOT / "bridge" / "mailbox"
INBOX = MAILBOX / "inbox.jsonl"
OUTBOX = MAILBOX / "outbox.jsonl"
STATUS = MAILBOX / "status.json"
PROCESSED = MAILBOX / "processed_ids.txt"
DIST = ROOT / "dist"


def _now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def ensure_mailbox() -> None:
    MAILBOX.mkdir(parents=True, exist_ok=True)
    for p in (INBOX, OUTBOX, PROCESSED):
        if not p.exists():
            p.write_text("", encoding="utf-8")


def write_status(**kwargs) -> None:
    ensure_mailbox()
    payload = {"ts": _now(), "watcher": "online", **kwargs}
    STATUS.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def append_outbox(row: dict) -> None:
    ensure_mailbox()
    with OUTBOX.open("a", encoding="utf-8") as f:
        f.write(json.dumps(row, ensure_ascii=False) + "\n")
    write_status(**row)


def seen_ids() -> set[str]:
    ensure_mailbox()
    return {line.strip() for line in PROCESSED.read_text(encoding="utf-8").splitlines() if line.strip()}


def mark_seen(cmd_id: str) -> None:
    with PROCESSED.open("a", encoding="utf-8") as f:
        f.write(cmd_id + "\n")


def run(cmd: list[str], cwd: Path | None = None, timeout: int = 600) -> tuple[bool, str]:
    try:
        proc = subprocess.run(
            cmd,
            cwd=str(cwd or ROOT),
            capture_output=True,
            text=True,
            timeout=timeout,
            env={**os.environ},
        )
        out = (proc.stdout or "") + (proc.stderr or "")
        tail = "\n".join(out.splitlines()[-40:])
        return proc.returncode == 0, tail
    except subprocess.TimeoutExpired:
        return False, f"timeout after {timeout}s: {' '.join(cmd)}"
    except FileNotFoundError as e:
        return False, str(e)


def find_adb() -> str | None:
    adb = shutil.which("adb")
    if adb:
        return adb
    home = os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT")
    if home:
        candidate = Path(home) / "platform-tools" / "adb"
        if candidate.exists():
            return str(candidate)
    return None


def latest_apk() -> Path | None:
    candidates = [
        ANDROID / "app" / "build" / "outputs" / "apk" / "release" / "app-release.apk",
        ANDROID / "app" / "build" / "outputs" / "apk" / "debug" / "app-debug.apk",
        DIST / "Boriken-Learner.apk",
    ]
    existing = [p for p in candidates if p.exists()]
    if not existing:
        return None
    return max(existing, key=lambda p: p.stat().st_mtime)


def handle(cmd: dict) -> dict:
    action = cmd.get("action")
    cmd_id = cmd.get("id", "unknown")
    base = {"id": cmd_id, "ts": _now(), "action": action, "source": cmd.get("source")}

    if action == "ping":
        return {**base, "ok": True, "message": "pong from studio_watcher"}

    if action == "git_pull":
        ok, tail = run(["git", "pull", "--ff-only"], cwd=ROOT)
        return {**base, "ok": ok, "message": "git pull", "log_tail": tail}

    if action == "sync_assets":
        if not WEBAPP.exists():
            return {**base, "ok": False, "message": f"missing {WEBAPP}"}
        ASSETS.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(WEBAPP, ASSETS)
        return {
            **base,
            "ok": True,
            "message": f"synced webapp → {ASSETS.relative_to(ROOT)}",
            "artifacts": [str(ASSETS.relative_to(ROOT))],
        }

    if action in {"build_debug", "build_release"}:
        # Always refresh assets before build
        if WEBAPP.exists():
            ASSETS.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(WEBAPP, ASSETS)
        task = "assembleDebug" if action == "build_debug" else "assembleRelease"
        gradlew = ANDROID / ("gradlew.bat" if os.name == "nt" else "gradlew")
        if not gradlew.exists():
            return {**base, "ok": False, "message": "gradlew missing — open android/ in Studio once"}
        ok, tail = run([str(gradlew), task, "--quiet"], cwd=ANDROID, timeout=900)
        arts: list[str] = []
        if ok and action == "build_release":
            apk = ANDROID / "app" / "build" / "outputs" / "apk" / "release" / "app-release.apk"
            if apk.exists():
                DIST.mkdir(parents=True, exist_ok=True)
                dest = DIST / "Boriken-Learner.apk"
                shutil.copy2(apk, dest)
                ver = DIST / "Boriken-Learner-0.3.2.apk"
                shutil.copy2(apk, ver)
                arts = [str(dest.relative_to(ROOT)), str(ver.relative_to(ROOT))]
        return {
            **base,
            "ok": ok,
            "message": "BUILD SUCCESSFUL" if ok else "BUILD FAILED",
            "artifacts": arts,
            "log_tail": tail,
        }

    if action == "adb_devices":
        adb = find_adb()
        if not adb:
            return {**base, "ok": False, "message": "adb not found — install platform-tools / set ANDROID_HOME"}
        ok, tail = run([adb, "devices", "-l"])
        return {**base, "ok": ok, "message": "adb devices", "log_tail": tail}

    if action == "adb_install":
        adb = find_adb()
        if not adb:
            return {**base, "ok": False, "message": "adb not found"}
        apk = latest_apk()
        if not apk:
            return {**base, "ok": False, "message": "no APK found — build_release first"}
        ok, tail = run([adb, "install", "-r", str(apk)], timeout=180)
        return {
            **base,
            "ok": ok,
            "message": f"adb install -r {apk.name}",
            "artifacts": [str(apk)],
            "log_tail": tail,
        }

    return {**base, "ok": False, "message": f"unknown action: {action}"}


def drain_once() -> int:
    ensure_mailbox()
    seen = seen_ids()
    if not INBOX.exists():
        return 0
    lines = INBOX.read_text(encoding="utf-8").splitlines()
    handled = 0
    for line in lines:
        if not line.strip():
            continue
        try:
            cmd = json.loads(line)
        except json.JSONDecodeError:
            continue
        cmd_id = str(cmd.get("id") or "")
        if not cmd_id or cmd_id in seen:
            continue
        write_status(ok=True, message=f"running {cmd.get('action')}", id=cmd_id, busy=True)
        result = handle(cmd)
        append_outbox(result)
        mark_seen(cmd_id)
        handled += 1
        print(json.dumps(result, ensure_ascii=False))
    return handled


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--watch", action="store_true", help="poll inbox forever")
    ap.add_argument("--interval", type=float, default=2.0)
    ap.add_argument("--once", action="store_true", help="process pending and exit")
    args = ap.parse_args()

    ensure_mailbox()
    write_status(ok=True, message="studio_watcher started", busy=False)

    if args.watch:
        print(f"[bridge] watching {INBOX}", flush=True)
        try:
            while True:
                drain_once()
                write_status(ok=True, message="idle", busy=False)
                time.sleep(args.interval)
        except KeyboardInterrupt:
            write_status(ok=True, message="watcher stopped", watcher="offline", busy=False)
            print("\n[bridge] stopped")
            return

    n = drain_once()
    write_status(ok=True, message=f"processed {n} command(s)", busy=False)
    if n == 0:
        print("[bridge] nothing pending")


if __name__ == "__main__":
    main()
