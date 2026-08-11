# Native Pure — Windows Desktop Companion

Desktop companion for the Native Pure dispensary phone app. Browse the menu, place pickup orders, manage stock, view customers (staff/admin), and manage account security on Windows.

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

## Features

- **18+ age gate** on first launch
- **Sign in / register** with the same role model as the Android app (Customer, Staff, Admin)
- **Menu, cart, pickup checkout** (8% tax estimate)
- **Orders** — customers see their own; staff/admin see all
- **Stock management** for staff/admin
- **Customer database** for staff/admin
- **Account security** — password policy, change password, login lockout, staff creation
- **Local hard drive storage** — inventory + customers auto-save on every change
  - Windows: `%LOCALAPPDATA%\NativePure\Companion\` (`store.json`, `inventory.json`, `customers.json`)
  - macOS/Linux: `~/.nativepure-companion/`
  - Account → **Open data folder** / **Save now**

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

Use the same JSON with the Android app Account → Desktop sync section. After import, data is written to the PC data folder above.
