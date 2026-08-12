# Gut Farms — Farm Manager (iOS)

Native SwiftUI companion to the Android `FarmManager` app.

## Features

- **Print reports** — Print farm report / profit report via the system printer sheet
- **Custom farm name** — tap the name on Home to rename
- **Livestock** — animal groups with expanded types
- **New animal arrivals** — acquire/birth date, registration status, optional name
- **Feeding schedules** — rations with daily/monthly cost projections
- **Breeding schedules** — matings with species gestation defaults and due dates
- **Profit margins** — income/expense ledger including projected feed cost
- **Data & maps** — upload KMZ / KML / CSV / JSON / images; add external API feed sources and pull live responses
- **Choice pickers** — lists with more than five options use a wheel (bubble) scroll

## Requirements

- macOS with **Xcode 15+**
- iOS 17.0+ device or simulator
- Apple ID for signing (Automatic)

## Download / distribute

iOS apps cannot be shared as a simple downloadable file like Android APKs.

- **Testers:** Archive in Xcode → upload to **TestFlight**
- **Public:** Archive → submit to the **App Store**

For the Android installable APK, see [`../DOWNLOAD.md`](../DOWNLOAD.md).

## Open & run

1. Pull branch `cursor/farm-management-android-115a`
2. Open `FarmManager-iOS/FarmManager.xcodeproj` in Xcode
3. Select your iPhone or a simulator
4. Set your **Team** under Signing & Capabilities if needed
5. Press **Run** (⌘R)

## Project layout

```
FarmManager-iOS/
  FarmManager.xcodeproj
  FarmManager/
    FarmManagerApp.swift
    Models/
    Views/
    Theme/
    Data/
    Assets.xcassets
```

## Notes

- Data is stored on-device with **SwiftData** (offline, seeded sample data on first launch)
- Android version lives in `../FarmManager/`
