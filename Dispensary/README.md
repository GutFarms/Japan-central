# Native Pure Dispensary (Android)

Adult-use dispensary app for browsing the menu, building a pickup bag, scanning inventory, and checking store info.

> **Download APK:** [`dist/Solstice-Dispensary.apk`](./dist/Solstice-Dispensary.apk) — see also [`../DOWNLOAD-DISPENSARY.md`](../DOWNLOAD-DISPENSARY.md).

## Features

- **21+ age gate** — required before entering the app
- **Customer accounts** — log in or create an account; local customer database with profile, phone, DOB, and marketing preference
- **Home** — branded welcome, category shortcuts, featured products
- **Menu** — flower, pre-rolls, edibles, concentrates, vapes, topicals, accessories with search and filters
- **Product detail** — THC/CBD, strain type, effects, quantity, add to bag
- **Bag & pickup checkout** — tax estimate, pickup name, order notes (linked to signed-in customer)
- **Orders** — local pickup history on device
- **Inventory + AI camera scanner** — CameraX + on-device ML Kit OCR/barcode scan to match SKUs or create products and add stock
- **Store** — hours, address, contact, pickup guidance

## Stack

- Kotlin + Jetpack Compose (Material 3)
- Room database (offline catalog + cart + orders + inventory)
- Navigation Compose + ViewModel
- CameraX + ML Kit text recognition & barcode scanning (on-device)

## Build

Requirements: JDK 17+, Android SDK 34

```bash
cd Dispensary
export ANDROID_HOME=$HOME/android-sdk   # or your SDK path
./gradlew assembleDebug
```

APK output: `app/build/outputs/apk/debug/app-debug.apk`

Release (signed if keystore present):

```bash
./gradlew assembleRelease
# output: app/build/outputs/apk/release/app-release.apk
```

Open the `Dispensary` folder in Android Studio to run on an emulator or device.

Requires **Android 8.0+** (API 26).
