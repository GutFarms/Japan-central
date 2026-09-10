# Download Gut Farms as an app

## Android (installable APK)

### Option A — Download the APK from this repo

1. Open [`FarmManager/dist/GutFarms-FarmManager.apk`](./FarmManager/dist/GutFarms-FarmManager.apk)
2. Download the file to your phone (or transfer via USB / Drive / AirDrop)
3. On Android: **Settings → Apps → Special access → Install unknown apps** → allow your browser/Files
4. Tap the APK → **Install** → open **Gut Farms**

Requires **Android 8.0+**.

### Option B — Update on the phone (no USB)

After the first sideload:

1. Open **Gut Farms → Home → Settings**
2. Tap **Check for updates**
3. If a newer build is published, tap **Download & install**
4. Allow **Install unknown apps** for Gut Farms if prompted

The app reads `update-manifest.json` from GitHub Releases tagged `gutfarms-v*`.

### Option C — GitHub Actions artifact

1. Open the repo **Actions** tab
2. Open the latest **Build downloadable Gut Farms APK** run
3. Download **GutFarms-FarmManager-apk**
4. Install as in Option A

### Option D — Versioned GitHub Release

Push a Gut Farms tag to publish a Release with the APK + update manifest:

```bash
git tag gutfarms-v1.3.3
git push origin gutfarms-v1.3.3
```

Then download from the repo **Releases** page, or use in-app **Check for updates**.

### Build the APK yourself

```bash
cd FarmManager
./gradlew assembleRelease
# output: app/build/outputs/apk/release/app-release.apk
# staged copy: dist/GutFarms-FarmManager.apk
```

### USB install from a computer (optional)

If the phone is connected with USB debugging:

```bash
adb install -r FarmManager/dist/GutFarms-FarmManager.apk
```

This cloud environment has **no phone attached**, so USB push has to be done from your machine.

---

## iOS (App Store / TestFlight)

Apple does **not** allow raw IPA sideloads like Android APKs for general users.

To distribute on iPhone:

1. Open `FarmManager-iOS/FarmManager.xcodeproj` in Xcode on a Mac
2. Sign with your Apple Developer team
3. Archive → distribute via **TestFlight** (testers) or **App Store** (public)

See [`FarmManager-iOS/README.md`](./FarmManager-iOS/README.md).
