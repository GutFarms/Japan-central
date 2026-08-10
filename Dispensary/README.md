# Solstice Dispensary (Android)

Adult-use dispensary app for browsing the menu, building a pickup bag, and checking store info.

> **Download APK:** [`dist/Solstice-Dispensary.apk`](./dist/Solstice-Dispensary.apk) — see also [`../DOWNLOAD-DISPENSARY.md`](../DOWNLOAD-DISPENSARY.md).

## Features

- **21+ age gate** — required before entering the app
- **Home** — branded welcome, category shortcuts, featured products
- **Menu** — flower, pre-rolls, edibles, concentrates, vapes, topicals, accessories with search and filters
- **Product detail** — THC/CBD, strain type, effects, quantity, add to bag
- **Bag & pickup checkout** — tax estimate, pickup name, order notes
- **Orders** — local pickup history on device
- **Store** — hours, address, contact, pickup guidance

## Stack

- Kotlin + Jetpack Compose (Material 3)
- Room database (offline catalog + cart + orders)
- Navigation Compose + ViewModel

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
