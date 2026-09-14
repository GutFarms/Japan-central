# Bridge protocol

Commands are JSON objects, one per line in `mailbox/inbox.jsonl`.

## Command

```json
{
  "id": "uuid-or-slug",
  "ts": "2026-09-14T01:00:00Z",
  "source": "cursor|studio|cli",
  "action": "ping|sync_assets|build_debug|build_release|adb_devices|adb_install|git_pull|status",
  "args": {},
  "note": "optional human note"
}
```

## Result (appended to `outbox.jsonl`, mirrored in `status.json`)

```json
{
  "id": "same-as-command",
  "ts": "2026-09-14T01:00:05Z",
  "action": "build_release",
  "ok": true,
  "message": "APK ready",
  "artifacts": ["dist/Boriken-Learner.apk"],
  "log_tail": "BUILD SUCCESSFUL"
}
```

## Actions

| action | Runs on Studio machine | Effect |
|---|---|---|
| `ping` | watcher | Heartbeat → status ok |
| `sync_assets` | watcher | Copy `webapp/index.html` → Android assets |
| `build_debug` | watcher | `./gradlew assembleDebug` |
| `build_release` | watcher | `./gradlew assembleRelease` + copy APK to `dist/` |
| `adb_devices` | watcher | List `adb devices` |
| `adb_install` | watcher | `adb install -r` latest release APK |
| `git_pull` | watcher | `git pull --ff-only` in repo root |
| `status` | either | Read `status.json` only |

Unknown actions fail with `ok: false`.
