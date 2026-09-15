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

## Android phone (sideload APK) — v0.3.3 fixed

**If download/install failed before, use this build.**

1. On your phone download **`Boriken-Learner.apk`** from Cursor Artifacts  
   (or open `install-android.html` in Artifacts and tap Download).
2. Chrome may say the file is unsafe → tap **Download anyway / Keep**.
3. Settings → Apps → Special app access → **Install unknown apps** → allow Chrome or Files.
4. Open **Downloads** → `Boriken-Learner.apk` → **Install** → **Open**.
5. Toast should say: **Borikén offline — classroom ready**.

Direct raw link (also OK after “Keep”):  
https://raw.githubusercontent.com/GutFarms/Japan-central/cursor/boriken-language-llm-404a/boriken-llm/dist/Boriken-Learner.apk

Uninstall any older Borikén build first. Package: `com.boriken.learner` · Android 7+.

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
