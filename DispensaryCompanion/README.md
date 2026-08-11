# Native Pure — Desktop Point of Sale

Windows / desktop **point of sale** for Native Pure. Staff ring in-store sales, look up loyalty customers, take cash or card tender, hand off phone pickup orders, and manage stock — all persisted on the local hard drive.

## Download

See [`../DOWNLOAD-DISPENSARY.md`](../DOWNLOAD-DISPENSARY.md).

GitHub Actions workflow **Build Native Pure Windows companion** uploads:

- **`NativePure-Companion-Windows`** — EXE / MSI + Windows uber JAR
- **`NativePure-Companion-linux-jar`** — cross-platform runnable JAR

Run a JAR with JDK 17+:

```bash
java -jar NativePure-Companion.jar
# or, after a local package:
./run-companion.sh
run-companion.bat
```

## POS features (staff / admin)

- **Register** — product search by name / SKU / brand, tap-to-add tiles, live ticket
- **Tender** — cash (with change) or card / other; voids clear the ticket
- **Loyalty** — look up customer by name, email, or phone; redeem 100 pts = $1; earn 1 pt per $1 paid
- **Pickup queue** — open phone/pickup orders on the register; hand off when customer arrives
- **Empty-state sample feed** — when no live orders exist, Register and Orders show a soft scrolling sample pickup feed (clears when real orders arrive)
- **Orders** — full history with channel (POS vs pickup) and payment method
- **Stock & customers** — same inventory / customer tools as before
- **Privacy** — customer PII is staff-only; lists/receipts mask email/phone/DOB; sync never exports password hashes; import cannot escalate roles; register idle-locks after 5 minutes
- **Local hard drive storage** — inventory + customers auto-save on every change
  - Windows: `%LOCALAPPDATA%\NativePure\Companion\` (`store.json`, `inventory.json`, `customers.json`)
  - `customers.json` backup omits password hashes
  - macOS/Linux: `~/.nativepure-companion/`
  - Account → **Open data folder** / **Save now** / **Lock register now**

Customer accounts still get menu browsing, cart, and pickup checkout.

## Default accounts (seeded)

| Role | Login | Password |
|------|--------|----------|
| Admin | `admin` or `fidelgutierrez33@gmail.com` | `12345678` |
| Demo customer | `demo@nativepure.example` | `demo1234` |

## Requirements

- **JDK 17+**
- Windows 10/11 for native `.exe` / `.msi` installers (or any OS to run the JVM app)

## Run from source

```bash
cd DispensaryCompanion
./gradlew run
```

On Windows:

```bat
cd DispensaryCompanion
gradlew.bat run
```

## Build installers

On a Windows machine with JDK 17+ (or via GitHub Actions):

```bat
cd DispensaryCompanion
gradlew.bat packageExe
gradlew.bat packageMsi
```

Outputs land under:

`build/compose/binaries/main/exe/`  
`build/compose/binaries/main/msi/`

Uber JAR for the current OS:

```bash
./gradlew packageUberJarForCurrentOS
mkdir -p dist
cp build/compose/jars/NativePureCompanion-*-*.jar dist/NativePure-Companion.jar
```

## Sync with the Android app

Staff/admin → **Account → Desktop sync**:

- **Export sync JSON** — `nativepure-sync-v1` (products + customers + orders)
- **Import sync JSON** — merges into the local hard-drive store (inventory + customers)

Use the same JSON with the Android app Account → Desktop sync section. Import phone pickup orders onto the POS queue, then hand them off from Register.
