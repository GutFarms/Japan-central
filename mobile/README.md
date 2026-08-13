# Njörðr Seas' CYD monitor (iPhone + Android)

Personal Expo app that pairs to **your** Windows Companion via a unique QR code.

## How pairing works

1. On the PC, open **Njörðr Seas' CYD miner → Settings → Phone monitor**.
2. Each Companion install generates a **personal QR** (unique pair id + secret token).
3. On the phone, tap **Scan personal QR**.
4. The app stores host + token and polls `http://<host>:19285/api/status` with your token.
5. Other users’ Companions reject your token — you only see your miner.

Also works in Safari / Chrome: use **Copy web link** from Companion (includes your token).

## Remote / travel

- Same Wi‑Fi: QR uses the PC LAN IP automatically.
- Around the globe: in Companion, set **Remote host** (DDNS, Tailscale IP, or public IP) and forward **TCP 19285** (or join the same VPN). The personal token still binds the phone to only that Companion.

**Regenerate QR** on the PC revokes every previously paired phone.

## Develop

```bash
cd mobile
npm install
npx expo start
```

- **Android:** press `a` or scan Metro QR with Expo Go  
- **iPhone:** Camera → Expo Go (same Wi‑Fi as Metro)

## Store builds

```bash
npx eas build --platform android
npx eas build --platform ios
```

Bundle IDs: `com.njordrseas.cydmonitor`
