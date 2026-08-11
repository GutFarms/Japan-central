# Native Pure Mail Server

Local mail stack for verification emails (register / login / resend).

## What it does

- HTTP API on **8787**
- Embedded SMTP catcher on **1025** (GreenMail) by default
- Web inbox at **http://127.0.0.1:8787/**
- Sends Native Pure verification codes via `POST /v1/mail/verification`

## Run

```bash
cd MailServer
./gradlew run
```

Optional env:

| Variable | Default | Meaning |
|----------|---------|---------|
| `MAIL_HTTP_PORT` | `8787` | API + inbox port |
| `SMTP_HOST` | `127.0.0.1` | SMTP host |
| `SMTP_PORT` | `1025` | SMTP port |
| `SMTP_FROM` | `noreply@nativepure.local` | From address |
| `SMTP_FROM_NAME` | `Native Pure` | From display name |
| `SMTP_USER` / `SMTP_PASS` | empty | Optional SMTP auth |
| `MAIL_EMBED_SMTP` | `true` | Start embedded GreenMail SMTP |

Point at a real SMTP provider by setting `MAIL_EMBED_SMTP=false` and your provider host/port/auth.

## API

```bash
# Health
curl http://127.0.0.1:8787/health

# Send verification code
curl -X POST http://127.0.0.1:8787/v1/mail/verification \
  -H 'Content-Type: application/json' \
  -d '{"to":"demo@nativepure.example","code":"123456","expiresInMinutes":15}'
```

Open **http://127.0.0.1:8787/** to read captured messages.

## App wiring

- Android / Companion call this API when issuing email codes.
- If the mail server is unreachable, apps still show the code on-device (offline fallback).

Default mail API base URL: `http://10.0.2.2:8787` on Android emulator, `http://127.0.0.1:8787` on desktop companion.
