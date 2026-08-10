# Native Pure — Windows Desktop Companion

Desktop companion for the Native Pure dispensary phone app. Browse the menu, place pickup orders, manage stock, view customers (staff/admin), and manage account security on Windows.

## Features

- **18+ age gate** on first launch
- **Sign in / register** with the same role model as the Android app (Customer, Staff, Admin)
- **Menu, cart, pickup checkout** (8% tax estimate)
- **Orders** — customers see their own; staff/admin see all
- **Stock management** for staff/admin
- **Customer database** for staff/admin
- **Account security** — password policy, change password, login lockout, staff creation
- Local data stored under `~/.nativepure-companion/store.json`

## Default accounts (seeded)

| Role | Login | Password |
|------|--------|----------|
| Admin | `admin` or `fidelgutierrez33@gmail.com` | `12345678` |
| Demo customer | `demo@nativepure.example` | `demo1234` |

## Requirements

- **JDK 17+**
- Windows 10/11 for native `.exe` / `.msi` installers (or any OS to run the JVM app)

## Run (any OS with JDK)

```bash
cd DispensaryCompanion
./gradlew run
```

On Windows:

```bat
cd DispensaryCompanion
gradlew.bat run
```

## Build Windows installers

On a Windows machine with JDK 17+ (or via GitHub Actions workflow **Build Native Pure Windows companion**):

```bat
cd DispensaryCompanion
gradlew.bat packageExe
gradlew.bat packageMsi
```

Outputs land under:

`build/compose/binaries/main/exe/`  
`build/compose/binaries/main/msi/`

You can also create a distributable app directory:

```bat
gradlew.bat createDistributable
```

Or a single runnable uber JAR for the current OS:

```bash
./gradlew packageUberJarForCurrentOS
# → build/compose/jars/NativePureCompanion-*-1.0.0.jar
```

## Sync with the Android app

Staff/admin → **Account → Desktop sync**:

- **Export sync JSON** — writes `nativepure-sync-v1` (products + orders snapshot)
- **Import sync JSON** — merges products by id (stock, publish state, prices)

Use the same JSON with the Android app Account → Desktop sync section.
