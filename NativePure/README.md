# Native Pure

Self-contained project for the **Native Pure** dispensary phone app and **desktop Point of Sale**.

| Path | What |
|------|------|
| [`android/`](./android/) | Android app (`com.solstice.dispensary`) |
| [`desktop/`](./desktop/) | Windows / desktop POS (`NativePureCompanion`) |
| [`mail-server/`](./mail-server/) | Local mail server for verification emails |
| [`DOWNLOADS.md`](./DOWNLOADS.md) | QR codes + download links |
| [`DOWNLOAD.md`](./DOWNLOAD.md) | Install instructions |
| [`downloads/`](./downloads/) | HTML poster + QR PNGs |

## Quick downloads

- **Android APK:** https://github.com/GutFarms/Japan-central/releases/latest/download/NativePure-Dispensary.apk
- **Windows POS Setup:** https://github.com/GutFarms/Japan-central/releases/latest/download/NativePure-POS-Setup.exe
- **SHA-256:** https://github.com/GutFarms/Japan-central/releases/latest/download/SHA256SUMS

## Build

```bash
# Android release APK
cd android && ./gradlew assembleRelease

# Desktop POS (dev)
cd desktop && ./gradlew run

# Mail server
cd mail-server && ./gradlew run
```

CI workflows live at the repository root under `.github/workflows/` and target these paths.
