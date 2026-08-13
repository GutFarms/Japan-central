# Native Pure — Desktop Point of Sale

Windows / desktop **point of sale** for Native Pure. Staff ring in-store sales, look up loyalty customers, take cash or card tender, hand off phone pickup orders, and manage stock — all persisted on the local hard drive.

## Download

See [`../DOWNLOAD.md`](../DOWNLOAD.md).

**Windows Setup (GitHub Releases):**  
https://github.com/GutFarms/Native/releases/latest/download/NativePure-POS-Setup.exe  

**SHA-256:**  
https://github.com/GutFarms/Native/releases/latest/download/SHA256SUMS  

GitHub Actions workflow **Build Native Pure Windows companion** uploads:

- **`NativePure-Companion-Windows`** — **`NativePure-POS-Setup.exe`** (logo install wizard) + `SHA256SUMS` + MSI/EXE/JAR
- **`NativePure-Companion-linux-jar`** — cross-platform runnable JAR + checksums

On `nativepure-v*` tags (and manual dispatch), the Setup EXE and checksums are also published to GitHub Releases. Optional Authenticode signing uses secrets `WINDOWS_CERT_PFX_BASE64` + `WINDOWS_CERT_PASSWORD`.

### Windows install wizard

1. Download **`NativePure-POS-Setup.exe`** and **`SHA256SUMS`**
2. Verify: `Get-FileHash .\NativePure-POS-Setup.exe -Algorithm SHA256`
3. Run the wizard (Native Pure logo sidebar / welcome screen)
4. Pick an install folder → Finish → launch from Start menu or desktop

Rebuild wizard art from the app logo:

```bash
./installer/generate-assets.sh
```

On Windows with [Inno Setup 6](https://jrsoftware.org/isinfo.php):

```bat
gradlew.bat createDistributable
"C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer\NativePurePOS-Setup.iss
:: → dist\NativePure-POS-Setup.exe
```

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
- **Privacy** — customer PII is staff-only; full reveal is **admin-only**; lists/receipts mask email/phone/DOB
- **Encrypted store** — `store.enc` (AES-GCM); plaintext `store.json` migrates automatically
- **PBKDF2 passwords** — upgraded from legacy SHA-256 on next login; sync never exports hashes
- **Sessions** — staff/admin must sign in after every restart; idle lock after 5 minutes; receipt clipboard clears in 30s
- **Bootstrap** — fresh installs write a one-time password to `admin-setup.txt` (delete after first login)
- **Local hard drive** — Windows `%LOCALAPPDATA%\NativePure\Companion\` / macOS-Linux `~/.nativepure-companion/`
  - `customers.json` backup omits hashes, DOB, notes; phones masked
  - Account → **Open data folder** / **Save now** / **Lock register now**

Customer accounts still get menu browsing, cart, and pickup checkout.

## Default accounts (seeded)

| Role | Login | Password |
|------|--------|----------|
| Admin (fresh install) | `admin` | See `admin-setup.txt` in the data folder (must change) |
| Demo customer | `demo@nativepure.example` | `demo1234` |

## Requirements

- **JDK 17+**
- Windows 10/11 for native `.exe` / `.msi` installers (or any OS to run the JVM app)

## Run from source

```bash
cd desktop
./gradlew run
```

On Windows:

```bat
cd desktop
gradlew.bat run
```

## Build installers

On a Windows machine with JDK 17+ (or via GitHub Actions):

```bat
cd desktop
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
