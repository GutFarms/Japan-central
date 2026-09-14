# Android — Borikén Learner (sideload APK)

Offline classroom app for phones. Packages the Borikén web learner
(Word of the Day, Batey Match, Island Time Machine, sentences) inside a
full-screen WebView. **No internet required after install.**

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
# refresh assets from the offline webapp
cp webapp/index.html android/app/src/main/assets/www/index.html
cd android
./gradlew assembleRelease
# APK:
#   app/build/outputs/apk/release/app-release.apk
```

`scripts/package.sh` also copies the release APK into `dist/` when this
folder builds successfully.
