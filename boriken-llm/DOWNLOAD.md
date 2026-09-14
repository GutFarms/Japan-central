# Download BorikenLLM

Packages (also under `dist/` after `./scripts/package.sh`):

| Package | Best for | File |
|---|---|---|
| **Android APK (sideload)** | Install on phone — offline classroom | `Boriken-Learner.apk` |
| **Android sideload zip** | APK + install steps | `Boriken-Android-Sideload.zip` |
| **Desktop (high graphics)** | Cinematic offline classroom on Linux | `Boriken-Desktop-linux.tar.gz` |
| **Offline Learner** | Instant play on iPhone/desktop — no server | `Boriken-Offline-Learner.zip` |
| **iOS Content Bundle** | Drop into Xcode (Swift kit + corpus + model) | `Boriken-iOS-ContentBundle.zip` |
| **Full Toolkit** | Run the API + retrain + ship the app | `BorikenLLM-Toolkit.zip` |

Agent downloads also land in `/opt/cursor/artifacts/`.

## Cursor ↔ Android Studio bridge

So Cloud Cursor and local Android Studio can cooperate:

1. On your PC (with Studio + SDK + optional phone):

```bash
cd boriken-llm
python3 bridge/studio_watcher.py --watch
```

2. In Android Studio: **File → Settings → Tools → External Tools → Import**  
   `bridge/studio_external_tools.xml`

3. From Cursor:

```bash
python3 bridge/cursor_to_studio.py enqueue build_release
python3 bridge/cursor_to_studio.py enqueue adb_install
python3 bridge/cursor_to_studio.py status
```

Full guide: [`bridge/README.md`](bridge/README.md)  
Official Cursor-in-JetBrains (ACP): https://cursor.com/docs/integrations/jetbrains

## Android phone (sideload APK)

1. Download **`Boriken-Learner.apk`** (or unzip `Boriken-Android-Sideload.zip`).
2. Copy it to your Android phone (Drive, USB, Bluetooth, Messages…).
3. On the phone: **Settings → Security / Apps → Install unknown apps**  
   and allow **Files**, **Chrome**, or **Drive**.
4. Tap **Boriken-Learner.apk → Install → Open**.
5. Play offline: Word of the Day, Batey Match, Island Time Machine, sentences.

- Package ID: `com.boriken.learner`
- Needs Android **7.0+**
- Signed with the debug keystore for immediate sideload (resign for Play Store)

Direct link (this branch):  
https://raw.githubusercontent.com/GutFarms/Japan-central/cursor/boriken-language-llm-404a/boriken-llm/dist/Boriken-Learner.apk

## Desktop (highest graphics)

```bash
tar -xzf Boriken-Desktop-linux.tar.gz
cd Boriken-Desktop-*-linux
./boriken-desktop
```

## Quickest path (iPhone)

1. Download **Boriken-Offline-Learner.zip**
2. Unzip → open `index.html` in Safari  
3. Share → **Add to Home Screen**

## Build packages yourself

```bash
cd boriken-llm
./scripts/package.sh
```
