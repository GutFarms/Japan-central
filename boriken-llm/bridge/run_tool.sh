#!/usr/bin/env bash
# Helper invoked by Android Studio External Tools.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
ACTION="${1:-status}"

case "$ACTION" in
  watch)
    exec python3 bridge/studio_watcher.py --watch
    ;;
  once)
    exec python3 bridge/studio_watcher.py --once
    ;;
  status)
    exec python3 bridge/cursor_to_studio.py status
    ;;
  ping|sync_assets|build_debug|build_release|adb_devices|adb_install|git_pull)
    python3 bridge/cursor_to_studio.py enqueue "$ACTION" --source studio --note "external-tool"
    exec python3 bridge/studio_watcher.py --once
    ;;
  *)
    echo "Unknown action: $ACTION" >&2
    exit 1
    ;;
esac
