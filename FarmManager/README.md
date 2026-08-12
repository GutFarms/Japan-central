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
- **Data & maps** — upload KMZ / KML / CSV / JSON / images; add external API feed sources (GET/POST) and pull live responses for later mapping
- **Home dashboard** — livestock head count, active feeds, breeding count, pending arrivals, margin snapshot, recent arrivals, upcoming due dates, and today's feeding list

## Stack

- Kotlin + Jetpack Compose (Material 3)
- Room database (offline-first, sample data on first launch)
- Navigation Compose + ViewModel

## Build

Requirements: JDK 17+, Android SDK 34

```bash
cd FarmManager
export ANDROID_HOME=$HOME/android-sdk   # or your SDK path
./gradlew assembleDebug
```

APK output: `app/build/outputs/apk/debug/app-debug.apk`

Open the `FarmManager` folder in Android Studio to run on an emulator or device.
