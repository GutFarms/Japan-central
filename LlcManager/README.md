# LLC Manager

Android app for managing multiple LLCs with deductions and inventory.

## Features

- **Multiple LLCs** — add, edit, and remove entities (name, EIN, state, notes)
- **Deductions** — quick add/remove per LLC with category and amount totals
- **Inventory** — item list with quantity adjust (+/−), unit cost, and value

Data is stored locally on the phone with Room (SQLite).

## Open in Android Studio

1. Open the `LlcManager` folder (File → Open).
2. Let Gradle sync finish.
3. Run on a phone or emulator (API 26+).

### USB install

1. Enable **Developer options** and **USB debugging** on the phone.
2. Plug in USB and accept the debugging prompt.
3. In Android Studio: Run ▶ `app`.

## Build from CLI

```bash
cd LlcManager
export ANDROID_HOME="$HOME/Android/Sdk"
./gradlew assembleDebug
```

APK output: `app/build/outputs/apk/debug/app-debug.apk`
