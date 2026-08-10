# Download Native Pure

Ready-to-install builds for the phone app and desktop companion.

## Android APK

**File:** `NativePure-Dispensary.apk` (also mirrored as `Solstice-Dispensary.apk`)

### Option A — GitHub Actions (always current)

1. Open **[Actions](../../actions)** → workflow **Build downloadable Native Pure Dispensary APK**
2. Open the latest green run → download artifact **`NativePure-Dispensary-apk`**
3. On the phone: allow install from that source (**Settings → Apps → Special access → Install unknown apps**)
4. Install the APK → open **Native Pure** → confirm **18+**

Requires **Android 8.0+**.

### Option B — From this repo

If present: [`Dispensary/dist/Solstice-Dispensary.apk`](./Dispensary/dist/Solstice-Dispensary.apk)

Prefer Option A when the file is missing or outdated (GitHub warns on files over 50MB).

### Build yourself

```bash
cd Dispensary
./gradlew assembleRelease
cp app/build/outputs/apk/release/app-release.apk dist/NativePure-Dispensary.apk
```

---

## Desktop companion (Windows / Mac / Linux)

### Option A — GitHub Actions (preferred)

1. Open **[Actions](../../actions)** → **Build Native Pure Windows companion**
2. Download either:
   - **`NativePure-Companion-Windows`** — `.exe` / `.msi` installers + Windows JAR
   - **`NativePure-Companion-linux-jar`** — runnable uber JAR (any OS with JDK 17+)

### Option B — Run the JAR locally

Needs **JDK 17+**.

```bash
# After downloading NativePure-Companion.jar
java -jar NativePure-Companion.jar
```

Helpers (after building):

```bash
DispensaryCompanion/run-companion.sh    # macOS / Linux
DispensaryCompanion/run-companion.bat   # Windows
```

### Build yourself

```bash
cd DispensaryCompanion
./gradlew packageUberJarForCurrentOS
# → build/compose/jars/NativePureCompanion-*-*.jar

# Windows only (on Windows or the Actions runner):
gradlew.bat packageExe packageMsi
```

See [`DispensaryCompanion/README.md`](./DispensaryCompanion/README.md).

---

## Default accounts (both apps)

| Role | Login | Password |
|------|--------|----------|
| Admin | `admin` or `fidelgutierrez33@gmail.com` | `12345678` (must change on first login) |
| Demo customer | `demo@nativepure.example` | `demo1234` |

## Phone ↔ desktop sync

Staff/admin → **Account → Desktop sync**: export/import `nativepure-sync-v1` JSON so inventory publish state and stock can be shared as a file.
