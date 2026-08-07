# Download Gut Farms as an app

## Android (installable APK)

### Option A — Download the APK from this repo

1. Open [`FarmManager/dist/GutFarms-FarmManager.apk`](./FarmManager/dist/GutFarms-FarmManager.apk)
2. Download the file to your phone (or transfer via USB/AirDrop/Drive)
3. On Android: **Settings → Apps → Special access → Install unknown apps** → allow your browser/Files
4. Tap the APK → **Install** → open **Gut Farms**

Requires **Android 8.0+**.

### Option B — GitHub Actions artifact

1. Open the repo **Actions** tab
2. Open the latest **Build downloadable Gut Farms APK** run
3. Download **GutFarms-FarmManager-apk**
4. Install as above

### Option C — Versioned GitHub Release

Push a tag to publish a Release with the APK attached:

```bash
git tag v1.1.0
git push origin v1.1.0
```

Then download from the repo **Releases** page.

### Build the APK yourself

```bash
cd FarmManager
./gradlew assembleRelease
# output: app/build/outputs/apk/release/app-release.apk
```

---

## iOS (App Store / TestFlight)

Apple does **not** allow raw IPA sideloads like Android APKs for general users.

To distribute on iPhone:

1. Open `FarmManager-iOS/FarmManager.xcodeproj` in Xcode on a Mac
2. Sign with your Apple Developer team
3. Archive → distribute via **TestFlight** (testers) or **App Store** (public)

See [`FarmManager-iOS/README.md`](./FarmManager-iOS/README.md).
