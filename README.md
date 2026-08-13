# Native

**Native Pure** — dispensary phone app and desktop Point of Sale.

Home: **https://github.com/GutFarms/Native**

| Path | What |
|------|------|
| [`android/`](./android/) | Android app (`com.solstice.dispensary`) |
| [`desktop/`](./desktop/) | Windows / desktop POS |
| [`mail-server/`](./mail-server/) | Verification mail server |
| [`DOWNLOADS.md`](./DOWNLOADS.md) | QR codes + download links |
| [`DOWNLOAD.md`](./DOWNLOAD.md) | Install instructions |

## Downloads

- **Android APK:** https://github.com/GutFarms/Native/releases/latest/download/NativePure-Dispensary.apk
- **Windows POS Setup:** https://github.com/GutFarms/Native/releases/latest/download/NativePure-POS-Setup.exe
- **SHA-256:** https://github.com/GutFarms/Native/releases/latest/download/SHA256SUMS

## Build

```bash
cd android && ./gradlew assembleRelease
cd desktop && ./gradlew run
cd mail-server && ./gradlew run
```

## Verified (clean)

- Desktop: `./gradlew clean test` (data package) — passed
- Android: `./gradlew clean assembleRelease` — **1.21.0** / versionCode **26**
- Mail server: `./gradlew clean compileKotlin` — passed
