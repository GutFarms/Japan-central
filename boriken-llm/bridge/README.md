# Cursor ↔ Android Studio Bridge

Two ways for **Cursor** and **Android Studio** to work together on Borikén Learner:

| Mode | What it does | Best when |
|---|---|---|
| **A. JetBrains ACP (official)** | Run the Cursor agent *inside* Android Studio AI Chat | You edit/build on your laptop in Studio |
| **B. Mailbox bridge (this repo)** | Shared command queue + watcher for sync / Gradle / ADB | Cloud Cursor + local Studio/device |

They complement each other. Use A for in-IDE agent chat; use B so a Cloud Agent can ask your laptop to build/install.

---

## A — Cursor agent inside Android Studio (ACP)

Android Studio is a JetBrains IDE. Cursor documents JetBrains ACP here:

- https://cursor.com/docs/integrations/jetbrains  
- https://cursor.com/docs/cli/acp  

### Setup (on your computer)

1. Cursor plan that includes agents (paid).
2. Android Studio with **AI Assistant** plugin (2025.1+ recommended).
3. Open **AI Chat** (right sidebar, or *View → Tool Windows → AI Chat*).
4. Agent provider → **Add Agent from Registry** → search **Cursor** → install.
5. Select **Cursor**, sign in.
6. Open this repo’s `boriken-llm/android` folder as the Studio project.
7. Prompt in AI Chat — the agent can edit files and run terminal commands *in Studio*.

Optional CLI ACP (any ACP client):

```bash
# On the machine with Cursor CLI installed
agent login
agent acp
```

---

## B — Mailbox bridge (Cloud Cursor ↔ local Studio)

### Layout

```
boriken-llm/bridge/
  README.md                 ← this file
  protocol.md               ← command schema
  mailbox/                  ← live queue (gitignored except .gitkeep)
    inbox.jsonl             ← commands waiting
    outbox.jsonl            ← results
    status.json             ← latest heartbeat / job
  cursor_to_studio.py       ← Cursor / CI posts commands here
  studio_watcher.py         ← run on the Android Studio machine
  studio_external_tools.xml ← import into Studio External Tools
```

### One-time setup on the Android Studio PC

```bash
cd boriken-llm
python3 -m pip install --user none  # stdlib only

# Keep this running while you develop (Terminal tool window is fine)
python3 bridge/studio_watcher.py --watch
```

Or import **External Tools** from `bridge/studio_external_tools.xml`:

*File → Settings → Tools → External Tools → Import*

Tools added:

- **Boriken: Sync web assets**
- **Boriken: Build release APK**
- **Boriken: ADB install**
- **Boriken: Bridge watcher**
- **Boriken: Bridge status**

### How Cloud Cursor talks to Studio

1. Cloud agent writes a command:

```bash
python3 bridge/cursor_to_studio.py enqueue build_release
python3 bridge/cursor_to_studio.py enqueue adb_install
python3 bridge/cursor_to_studio.py status
```

2. Local `studio_watcher.py` picks it up, runs Gradle/ADB, writes `status.json` + outbox.
3. Cursor (or you) reads status:

```bash
python3 bridge/cursor_to_studio.py status --watch 60
```

### Typical loop

```
Cursor (cloud) edits webapp / android sources
        │  git push
        ▼
You pull in Android Studio  (or watcher auto-pulls if --git-pull)
        │
        ▼
Cursor enqueues: sync_assets → build_release → adb_install
        │
        ▼
studio_watcher runs on laptop with phone via USB/Wi‑Fi ADB
        │
        ▼
status.json = ok + apk path  →  Cursor confirms
```

---

## Open the project in Android Studio

1. Clone / pull `cursor/boriken-language-llm-404a`.
2. **File → Open** → select `boriken-llm/android` (the folder with `settings.gradle`).
3. Let Gradle sync. Set SDK if prompted (`local.properties` is gitignored).
4. Run configuration: app `com.boriken.learner` → device/emulator → Run.

Sync learner HTML before Run:

```bash
cd boriken-llm
cp webapp/index.html android/app/src/main/assets/www/index.html
# or:
python3 bridge/cursor_to_studio.py enqueue sync_assets --wait
```

---

## Wireless ADB (phone ↔ Studio PC)

```bash
adb pair <phone-ip>:<pair-port>     # pairing code from phone
adb connect <phone-ip>:<debug-port>
adb devices
python3 bridge/cursor_to_studio.py enqueue adb_install
```

Cloud Cursor cannot see your USB phone; the **watcher on your PC** is the link.

---

## Security notes

- Mailbox files stay local (`bridge/mailbox/` is gitignored).
- Do not commit device serials or signing passwords into inbox commands.
- Release APK here still uses the debug keystore for sideload; resign for Play Store.
