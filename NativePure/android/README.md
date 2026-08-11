# Native Pure Dispensary (Android)

Adult-use dispensary app for browsing the menu, building a pickup bag, scanning inventory, and checking store info.

> **Download APK:** [`dist/NativePure-Dispensary.apk`](./dist/NativePure-Dispensary.apk) — also on [GitHub Releases](https://github.com/GutFarms/Japan-central/releases).  
> **QR codes:** [`../DOWNLOADS.md`](../DOWNLOADS.md) · [`dist/qr/nativepure-android.png`](./dist/qr/nativepure-android.png)  
> Steps: [`../DOWNLOAD.md`](../DOWNLOAD.md).
>
> **Updates:** After install, the app checks GitHub Releases for a newer `update-manifest.json` and can download/install the APK in-app (Account → Check for updates). Requires a public release asset (or network access to that URL). Auto-check also runs daily at 1:00 AM.

## Features

- **18+ age gate** — required before entering the app
- **Customer accounts** — log in or create an account; customers only see their own profile/orders
- **Admin & staff** — main admin `admin` / `fidelgutierrez33@gmail.com` (fresh installs show a one-time password on sign-in — change immediately); admin can create staff sub-accounts by email; admin/staff can view customer data (full PII reveal is admin-only)
- **Security** — PBKDF2 passwords, random bootstrap admin password (not hardcoded), password policy, email verification, login lockout, optional app PIN lock with auto-lock, forced password change for staff temp passwords, cart cleared on logout
- **Keyboard-safe forms** — auth, verification, password, and account screens scroll above the IME so typed text stays visible
- **Light / Dark mode** — System, Light, or Dark (Account → Appearance)
- **Home** — branded welcome, category shortcuts, featured products
- **Menu** — flower, pre-rolls, edibles, concentrates, vapes, topicals, accessories with search and filters
- **Product detail** — THC/CBD, strain type, effects, quantity, add to bag
- **Bag & pickup checkout** — tax estimate, pickup name, order notes (linked to signed-in customer)
- **Orders** — local pickup history on device
- **Inventory + AI camera scanner** — CameraX + on-device ML Kit OCR/barcode scan to match SKUs or create products and add stock; new scans start unpublished until staff publishes them
- **Published catalog** — customers only see published products; publish requires SKU + price; drafts/AI scans stay in Stock
- **Staff controls** — admin can list, disable/enable, and reset staff passwords
- **Encrypted session prefs** + forced bootstrap admin password change
- **Inventory sync JSON** — export/import between Android and Windows POS (no password hashes)
- **Store** — hours, address, contact, pickup guidance
- **Plant-inspired UI** — canopy greens, Fraunces + Nunito Sans, botanical hero/age-gate motion, leaf empty states
- **Windows desktop POS** — logo install wizard; see [`../desktop/README.md`](../desktop/README.md)
## Stack

- Kotlin + Jetpack Compose (Material 3)
- Room database (offline catalog + cart + orders + inventory)
- Navigation Compose + ViewModel
- CameraX + ML Kit text recognition & barcode scanning (on-device)

## Build

Requirements: JDK 17+, Android SDK 34

```bash
cd NativePure/android
export ANDROID_HOME=$HOME/android-sdk   # or your SDK path
./gradlew assembleDebug
```

APK output: `app/build/outputs/apk/debug/app-debug.apk`

Release (signed if keystore present):

```bash
./gradlew assembleRelease
# output: app/build/outputs/apk/release/app-release.apk
```

Open the `NativePure/android` folder in Android Studio to run on an emulator or device.

Requires **Android 8.0+** (API 26).
