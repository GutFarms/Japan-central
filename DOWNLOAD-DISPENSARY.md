# Download Solstice Dispensary

## Android (installable APK)

### Option A — Download the APK from this repo

1. Open [`Dispensary/dist/Solstice-Dispensary.apk`](./Dispensary/dist/Solstice-Dispensary.apk)
2. Download the file to your phone
3. On Android: **Settings → Apps → Special access → Install unknown apps** → allow your browser/Files
4. Tap the APK → **Install** → open **Solstice**

Requires **Android 8.0+**. You must confirm you are 21+ on first launch.

### Option B — GitHub Actions artifact

1. Open the repo **Actions** tab
2. Open the latest **Build downloadable Solstice Dispensary APK** run
3. Download **Solstice-Dispensary-apk**
4. Install as above

### Build yourself

```bash
cd Dispensary
./gradlew assembleRelease
# output: app/build/outputs/apk/release/app-release.apk
```
