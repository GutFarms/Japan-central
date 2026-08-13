# Download Native Pure

## QR codes (scan to download)

See **[`DOWNLOADS.md`](./DOWNLOADS.md)** for printable QR codes:

| App | QR |
|-----|-----|
| Android APK | [`downloads/qr/nativepure-android.png`](./downloads/qr/nativepure-android.png) |
| Desktop POS | [`downloads/qr/nativepure-companion.png`](./downloads/qr/nativepure-companion.png) |

Local HTML poster: [`downloads/index.html`](./downloads/index.html)

---

## Android APK (download)

**Scan or open the latest release APK:**  
https://github.com/GutFarms/Native/releases/latest/download/NativePure-Dispensary.apk

**Direct file (this branch):**  
[`android/dist/NativePure-Dispensary.apk`](./android/dist/NativePure-Dispensary.apk)

**GitHub Releases:**  
https://github.com/GutFarms/Native/releases  
→ download **`NativePure-Dispensary.apk`**

### Install on phone

1. Download the APK to your Android device (or scan the Android QR)
2. **Settings → Apps → Special access → Install unknown apps** → allow your browser/Files
3. Open the APK → install → open **Native Pure** → confirm **18+**

Requires **Android 8.0+**.

### Also from Actions

1. **[Actions](../../actions)** → **Build downloadable Native Pure Dispensary APK** (`nativepure-android-release`)
2. Latest green run → artifact **`NativePure-Dispensary-apk`**

### Build yourself

```bash
cd android
./gradlew assembleRelease
cp app/build/outputs/apk/release/app-release.apk dist/NativePure-Dispensary.apk
```

---

## Desktop POS (Windows / Mac / Linux)

Point-of-sale register for in-store sales, loyalty lookup, cash/card tender, and phone pickup handoff.

### Windows — install wizard (recommended)

**Stable download (from the latest GitHub Release):**  
https://github.com/GutFarms/Native/releases/latest/download/NativePure-POS-Setup.exe

**Checksums:**  
https://github.com/GutFarms/Native/releases/latest/download/SHA256SUMS

Verify in PowerShell:

```powershell
Get-FileHash .\NativePure-POS-Setup.exe -Algorithm SHA256
Get-Content .\SHA256SUMS
```

1. Download **`NativePure-POS-Setup.exe`** (+ `SHA256SUMS`)
2. Confirm the SHA-256 hash matches
3. Run the branded install wizard with the Native Pure logo
4. Choose install folder → Finish → launch from Start menu or desktop shortcut

Also available from **[Actions](../../actions)** → **Build Native Pure Windows companion** (`nativepure-windows-pos`) → artifact **`NativePure-Companion-Windows`** (includes MSI/EXE/JAR).

Optional Authenticode signing uses repo secrets `WINDOWS_CERT_PFX_BASE64` + `WINDOWS_CERT_PASSWORD` when configured.

### Mac / Linux — runnable JAR

1. Same Actions workflow → artifact **`NativePure-Companion-linux-jar`**
2. Needs **JDK 17+**:

```bash
java -jar NativePure-Companion.jar
```

Helpers after a local build:

```bash
desktop/run-companion.sh    # macOS / Linux
desktop/run-companion.bat   # Windows
```

### Build yourself

```bash
cd desktop
./gradlew createDistributable
# Windows (with Inno Setup 6 installed):
#   ISCC installer/NativePurePOS-Setup.iss
# → dist/NativePure-POS-Setup.exe

./gradlew packageUberJarForCurrentOS
# → build/compose/jars/NativePureCompanion-*-*.jar
```

See [`desktop/README.md`](./desktop/README.md).

---

## Default accounts (both apps)

| Role | Login | Password |
|------|--------|----------|
| Admin (desktop fresh install) | `admin` | See `admin-setup.txt` in the POS data folder (must change) |
| Admin (Android fresh install) | `admin` | One-time password shown on the sign-in screen (must change) |
| Demo customer | `demo@nativepure.example` | `demo1234` |

## Phone ↔ desktop sync

Staff/admin → **Account → Desktop sync**: export/import `nativepure-sync-v1` JSON (products + customers + orders). On the PC POS, imported data is saved to the local hard drive (`%LOCALAPPDATA%\NativePure\Companion\` on Windows). Phone pickup orders appear in the Register pickup queue after import.
