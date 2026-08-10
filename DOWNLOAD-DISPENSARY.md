# Download Native Pure Dispensary

## Android (installable APK)

### Option A — GitHub Actions artifact (preferred)

1. Open the repo **Actions** tab
2. Open the latest **Build downloadable Native Pure Dispensary APK** run
3. Download **Solstice-Dispensary-apk**
4. On Android: **Settings → Apps → Special access → Install unknown apps** → allow your browser/Files
5. Install → open **Native Pure**

Requires **Android 8.0+**. Confirm you are **18+** on first launch.

### Option B — APK in this repo

[`Dispensary/dist/Solstice-Dispensary.apk`](./Dispensary/dist/Solstice-Dispensary.apk) may be present for convenience. Prefer Option A when the file is missing or outdated (GitHub warns on files over 50MB).

### Build yourself

```bash
cd Dispensary
./gradlew assembleRelease
# output: app/build/outputs/apk/release/app-release.apk
```

## Windows desktop companion

```bash
cd DispensaryCompanion
./gradlew run
# On Windows:
gradlew.bat packageExe
gradlew.bat packageMsi
```

Installers are also produced by the **Build Native Pure Windows companion** GitHub Action.

See [`DispensaryCompanion/README.md`](./DispensaryCompanion/README.md).

### Sync phone ↔ desktop

Staff/admin can **Export / Import inventory sync JSON** (`nativepure-sync-v1`) from Account on either app so stock and publish state can be shared as a file.
