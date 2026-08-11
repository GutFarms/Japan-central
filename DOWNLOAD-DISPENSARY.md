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
https://github.com/GutFarms/Japan-central/releases/latest/download/NativePure-Dispensary.apk

**Direct file (this branch):**  
[`Dispensary/dist/NativePure-Dispensary.apk`](./Dispensary/dist/NativePure-Dispensary.apk)

**GitHub Releases:**  
https://github.com/GutFarms/Japan-central/releases  
→ download **`NativePure-Dispensary.apk`**

### Install on phone

1. Download the APK to your Android device (or scan the Android QR)
2. **Settings → Apps → Special access → Install unknown apps** → allow your browser/Files
3. Open the APK → install → open **Native Pure** → confirm **18+**

Requires **Android 8.0+**.

### Also from Actions

1. **[Actions](../../actions)** → **Build downloadable Native Pure Dispensary APK**
2. Latest green run → artifact **`NativePure-Dispensary-apk`**

### Build yourself

```bash
cd Dispensary
./gradlew assembleRelease
cp app/build/outputs/apk/release/app-release.apk dist/NativePure-Dispensary.apk
```

---

## Desktop POS (Windows / Mac / Linux)

Point-of-sale register for in-store sales, loyalty lookup, cash/card tender, and phone pickup handoff.

### Windows — install wizard (recommended)

1. Open **[Actions](../../ed)** → **Build Native Pure Windows companion**
2. Open the latest green run → download **`NativePure-Companion-Windows`**
3. Run **`NativePure-POS-Setup.exe`** — branded install wizard with the Native Pure logo
4. Choose install folder → Finish → launch from Start menu or desktop shortcut

Also in that artifact (optional): Compose `.exe` / `.msi` packages and a Windows uber JAR.

### Mac / Linux — runnable JAR

1. Same Actions workflow → artifact **`NativePure-Companion-linux-jar`**
2. Needs **JDK 17+**:

```bash
java -jar NativePure-Companion.jar
```

Helpers after a local build:

```bash
DispensaryCompanion/run-companion.sh    # macOS / Linux
DispensaryCompanion/run-companion.bat   # Windows
```

### Build yourself

```bash
cd DispensaryCompanion
./gradlew createDistributable
# Windows (with Inno Setup 6 installed):
#   ISCC installer/NativePurePOS-Setup.iss
# → dist/NativePure-POS-Setup.exe

./gradlew packageUberJarForCurrentOS
# → build/compose/jars/NativePureCompanion-*-*.jar
```

See [`DispensaryCompanion/README.md`](./DispensaryCompanion/README.md).

---

## Default accounts (both apps)

| Role | Login | Password |
|------|--------|----------|
| Admin | `admin` or `fidelgutierrez33@gmail.com` | `12345678` (must change on first login) |
| Demo customer | `demo@nativepure.example` | `demo1234` |

## Phone ↔ desktop sync

Staff/admin → **Account → Desktop sync**: export/import `nativepure-sync-v1` JSON (products + customers + orders). On the PC POS, imported data is saved to the local hard drive (`%LOCALAPPDATA%\NativePure\Companion\` on Windows). Phone pickup orders appear in the Register pickup queue after import.
