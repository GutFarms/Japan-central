# Njörðr Seas' CYD monitor (iPhone + Android)

Cross-platform Expo app that polls the Windows Companion phone-monitor API.

## How it works

1. Run **Njörðr Seas' CYD miner** on the PC (Companion listens on **TCP 19285**).
2. Phone and PC on the same Wi‑Fi.
3. Open this app → enter the PC LAN IP → live hashrate, pool, accepts, boards.

Also available without installing this app: open  
`http://<pc-lan-ip>:19285/` in Safari / Chrome (mobile web UI served by Companion).

## Develop

```bash
cd mobile
npm install
npx expo start
```

- **Android:** press `a` or scan QR with Expo Go  
- **iPhone:** scan QR with Camera → Expo Go (same Wi‑Fi as Metro)

## Store builds

```bash
# Requires Expo account + native credentials
npx eas build --platform android
npx eas build --platform ios
```

Bundle IDs:

- iOS: `com.njordrseas.cydmonitor`
- Android: `com.njordrseas.cydmonitor`

Cleartext LAN HTTP is enabled so the app can reach `http://192.168.x.x:19285`.
