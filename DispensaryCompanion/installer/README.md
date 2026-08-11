# Native Pure POS — Windows installer

Branded **install wizard** built with [Inno Setup](https://jrsoftware.org/isinfo.php) and the Native Pure logo.

| File | Purpose |
|------|---------|
| `NativePurePOS-Setup.iss` | Wizard script (welcome, logo panel, shortcuts) |
| `assets/icon.ico` | Setup + Start Menu / desktop icon |
| `assets/wizard-modern.bmp` | Large wizard sidebar (logo) |
| `assets/wizard-small.bmp` | Small wizard header icon |
| `generate-assets.sh` | Rebuild art from `src/main/resources/logo_main.png` |

CI produces **`dist/NativePure-POS-Setup.exe`** on every Windows companion build.
