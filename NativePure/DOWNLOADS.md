# Native Pure — download QR codes

Scan these codes to install the apps. Print this page for the counter or lobby.

## Android phone app

**Scan to download the APK** (opens the latest release):

<img src="./downloads/qr/nativepure-android.png" alt="QR code — download Native Pure Android APK" width="240" />

**Link:** https://github.com/GutFarms/Japan-central/releases/latest/download/NativePure-Dispensary.apk

### Install steps
1. Scan the QR with your phone camera  
2. Download **NativePure-Dispensary.apk**  
3. Allow install from that source if prompted  
4. Open **Native Pure** → confirm **18+**

Requires Android 8.0+.

---

## Desktop POS

<a id="desktop-companion"></a>
<a id="desktop-pos"></a>

**Scan for desktop POS download instructions** (Windows install wizard / Mac / Linux):

<img src="./downloads/qr/nativepure-companion.png" alt="QR code — Native Pure desktop POS downloads" width="240" />

**Link:** [DOWNLOAD.md — Desktop POS](./DOWNLOAD.md#desktop-pos-windows--mac--linux)

### Windows (easiest)

**Download:** https://github.com/GutFarms/Japan-central/releases/latest/download/NativePure-POS-Setup.exe  

**Checksums:** https://github.com/GutFarms/Japan-central/releases/latest/download/SHA256SUMS  

```powershell
Get-FileHash .\NativePure-POS-Setup.exe -Algorithm SHA256
```

Also from GitHub Actions → **Build Native Pure Windows POS** (`nativepure-windows-pos`) → artifact **`NativePure-Companion-Windows`**:

- **`NativePure-POS-Setup.exe`** — logo-branded install wizard (recommended)
- **`SHA256SUMS`** / **`SHA256SUMS.txt`** — integrity hashes for Windows `Get-FileHash`
- Optional: MSI / Compose EXE / JAR

### Mac / Linux

Artifact **`NativePure-Companion-linux-jar`** — runnable JAR (JDK 17+)

---

## All downloads (this page)

<img src="./downloads/qr/downloads-page.png" alt="QR code — Native Pure downloads page" width="200" />

Open: https://github.com/GutFarms/Japan-central/blob/master/DOWNLOADS.md

---

## PNG files

| App | File |
|-----|------|
| Android APK | [`downloads/qr/nativepure-android.png`](./downloads/qr/nativepure-android.png) |
| Companion | [`downloads/qr/nativepure-companion.png`](./downloads/qr/nativepure-companion.png) |
| This page | [`downloads/qr/downloads-page.png`](./downloads/qr/downloads-page.png) |

Also copied for sideload kits: [`android/dist/qr/nativepure-android.png`](./android/dist/qr/nativepure-android.png)

---

## Default logins

| Role | Login | Password |
|------|--------|----------|
| Admin (desktop fresh install) | `admin` | See `admin-setup.txt` in the POS data folder |
| Admin (Android fresh install) | `admin` | One-time password shown on sign-in (must change) |
| Demo | `demo@nativepure.example` | `demo1234` |
