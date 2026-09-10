# Gut Farms — Farm Manager (Android)

Android app for farm management with livestock tracking, feeding schedules, and profit margin analysis.

> **iOS version:** see [`../FarmManager-iOS/`](../FarmManager-iOS/) (SwiftUI + SwiftData).
>
> **Download Android APK:** [`dist/GutFarms-FarmManager.apk`](./dist/GutFarms-FarmManager.apk) — see also [`../DOWNLOAD.md`](../DOWNLOAD.md).

## Features

- **Print reports** — send a full farm report or profit report to a printer (system print sheet)
- **Custom farm name** — tap the farm name on Home to rename; the name appears on every screen header and is saved on-device
- **Livestock** — manage animal groups (cattle/dairy/beef, poultry, goat, sheep, pig, equine, rabbit, camelids, bison, buffalo, deer, ratites, fish, bees, and other) with head count and purchase cost
- **New animal arrivals** — separate screen for purchases, births, and transfers with acquire/birth date, registration status, optional name/tag ID
- **Feeding schedules** — timed rations per group with frequency, kg amounts, and cost-per-kg; projected daily/monthly feed cost
- **Breeding schedules** — mating/AI records with status, sire, expected offspring, and due dates (gestation defaults by species)
- **Profit margins** — income/expense ledger; net profit and margin % including projected monthly feed from active schedules
- **Home dashboard** — livestock head count, active feeds, breeding count, pending arrivals, margin snapshot, recent arrivals, upcoming due dates, today's feeding list, and a records hub entry
- **Feeding schedules** — quantity per feeding with units (kg/lb/bag/scoop/bale/gallon), animals fed, stock on hand, and projected cost
- **Farm records** — farm info, health log, inventory with reorder alerts, journal, and contacts

## Stack

- Kotlin + Jetpack Compose (Material 3)
- Room database (offline-first, sample data on first launch)
- Navigation Compose + ViewModel

## Install on a phone via Android Studio

1. On the phone: enable **Developer options** → turn on **USB debugging**
2. Plug the phone into your computer (accept the “Allow USB debugging?” prompt)
3. Open **Android Studio** → **File → Open** → select the `FarmManager` folder (not the repo root)
4. Wait for Gradle sync to finish
5. Top toolbar: pick your phone in the device dropdown
6. Click **Run** (green ▶) or press **Shift+F10** / **Ctrl+R**

Android Studio builds and installs/updates the app on the phone (`com.gutfarms.manager`).

**Release APK (no Studio):** use [`dist/GutFarms-FarmManager.apk`](./dist/GutFarms-FarmManager.apk), or after install use **Home → Settings → Check for updates**.

## Build

Requirements: JDK 17+, Android SDK 34

```bash
cd FarmManager
export ANDROID_HOME=$HOME/android-sdk   # or your SDK path
./gradlew assembleDebug
```

APK output: `app/build/outputs/apk/debug/app-debug.apk`

Open the `FarmManager` folder in Android Studio to run on an emulator or device.
