# Native Pure — Android downloads

CI uploads **`NativePure-Dispensary.apk`** (and a `Solstice-Dispensary.apk` alias) as the GitHub Actions artifact **`NativePure-Dispensary-apk`**.

Build locally:

```bash
cd Dispensary
./gradlew assembleRelease
cp app/build/outputs/apk/release/app-release.apk dist/NativePure-Dispensary.apk
```

See [`../../DOWNLOAD-DISPENSARY.md`](../../DOWNLOAD-DISPENSARY.md).
