# Native Pure POS — Windows installer

Branded **install wizard** built with [Inno Setup](https://jrsoftware.org/isinfo.php) and the Native Pure logo.

| File | Purpose |
|------|---------|
| `NativePurePOS-Setup.iss` | Wizard script (welcome, logo panel, shortcuts) |
| `assets/icon.ico` | Setup + Start Menu / desktop icon |
| `assets/wizard-modern.bmp` | Large wizard sidebar (logo) |
| `assets/wizard-small.bmp` | Small wizard header icon |
| `generate-assets.sh` | Rebuild art from `src/main/resources/logo_main.png` |

CI produces **`dist/NativePure-POS-Setup.exe`** plus **`SHA256SUMS`** on every Windows companion build.

Release tags `nativepure-v*` publish the Setup EXE and checksums to GitHub Releases (same release family as the Android APK).

### Optional Authenticode signing

Configure repository secrets:

| Secret | Value |
|--------|--------|
| `WINDOWS_CERT_PFX_BASE64` | Base64-encoded `.pfx` code-signing certificate |
| `WINDOWS_CERT_PASSWORD` | PFX password |

When unset, the build still succeeds and records `SIGNING-STATUS.txt` as `unsigned`.
