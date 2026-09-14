# Android — Borikén Learner (sideload APK)

Offline classroom app for phones. Packages the Borikén web learner
(Word of the Day, Batey Match, Island Time Machine, sentences) inside a
full-screen WebView. **No internet required after install.**

## Cursor ↔ Android Studio

See **[`../bridge/README.md`](../bridge/README.md)** for two-way setup:

1. **JetBrains ACP** — run the Cursor agent inside Android Studio AI Chat  
   (https://cursor.com/docs/integrations/jetbrains)
2. **Mailbox bridge** — Cloud Cursor enqueues build/install jobs; your PC
   runs `python3 bridge/studio_watcher.py --watch` (or Studio External Tools
   imported from `bridge/studio_external_tools.xml`).

Quick local watcher:

```bash
cd boriken-llm
python3 bridge/studio_watcher.py --watch
```

From Cursor (cloud or CLI):

```bash
python3 bridge/cursor_to_studio.py enqueue sync_assets
python3 bridge/cursor_to_studio.py enqueue build_release
python3 bridge/cursor_to_studio.py enqueue adb_install
python3 bridge/cursor_to_studio.py status
```

## Open in Android Studio

**File → Open** → select this `android/` folder (the one with `settings.gradle`).
Allow Gradle sync. Create/run the app on an emulator or USB/Wi‑Fi device.

## Download / install on your phone

1. Get **`dist/Boriken-Learner.apk`** (or `Boriken-Android-Sideload.zip`).
2. Send the APK to your Android phone (Drive, USB, Messages, etc.).
3. Open **Settings → Security / Apps → Install unknown apps** and allow the
   app you used to open the file (Files, Chrome, Drive…).
4. Tap **Boriken-Learner.apk → Install → Open**.

| Field | Value |
|---|---|
| Application ID | `com.boriken.learner` |
| Min Android | 7.0 (API 24) |
| Version | 0.3.2 |
| Signing | Debug keystore (sideload-ready; resign for Play Store) |

## Build from source

Needs JDK 17+, Android SDK 34, and Gradle wrapper (checked in).

```bash
export ANDROID_HOME=/path/to/android-sdk
cd boriken-llm
cp webapp/index.html android/app/src/main/assets/www/index.html
cd android
./gradlew assembleRelease
# APK: app/build/outputs/apk/release/app-release.apk
```

`scripts/package.sh` also copies the release APK into `dist/` when this
folder builds successfully.
