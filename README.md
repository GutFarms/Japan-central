# Cursor Appliance

**Version 0.4.1** — a lightweight, dedicated **local** [My Machines](https://cursor.com/docs/cloud-agent/self-hosted-guides/my-machines) worker for this repo, with an **egui** control panel, **local failover**, and a **Windows portable sandbox** build.

Cursor keeps planning/inference in the cloud. Tool calls (shell, edits, browser, local MCP) run on **this machine**. No inbound ports or VPN required.

Use this when you want a always-on local box (laptop, mini PC, or Raspberry Pi) that shows up as `cursor-appliance` on [cursor.com/agents](https://cursor.com/agents).

## What you get

| Piece | Role |
|---|---|
| `gui/` | egui desktop app + `cursor-local-worker` |
| `Start-CursorAppliance.bat` | Windows one-click launcher |
| `Start-PortableSandbox.bat` | Windows portable / Windows Sandbox launcher |
| `scripts/windows/*.ps1` | Windows setup / cloud / local / sandbox / status |
| `scripts/run-gui.sh` | Linux build + launch control panel |
| `scripts/setup.sh` | Linux Cursor Agent CLI bootstrap |
| `scripts/start-worker.sh` | Linux My Machines cloud worker |
| `scripts/start-local-worker.sh` | Linux local takeover worker |
| `scripts/package-windows.sh` | Assemble portable Windows zip |
| `systemd/cursor-appliance.service` | Linux boot service for cloud worker |
| Cloud `:8733` / Local `:8734` | `/healthz` `/readyz` `/status` |

This is **not** an Enterprise Self-Hosted Pool and **not** a Cursor-managed cloud VM. It is a personal My Machines appliance.

## Windows download (portable)

1. Grab **`cursor-appliance-windows-x64-v*.zip`** from  
   [GitHub Releases](https://github.com/GutFarms/Japan-central/releases)  
   (or the `cursor-appliance-windows-x64` Actions artifact).
2. Unzip anywhere.
3. Double-click **`Start-CursorAppliance.bat`**.

Full Windows notes: [WINDOWS.md](./WINDOWS.md).

```powershell
.\scripts\windows\Setup.ps1
.\Start-CursorAppliance.bat
# or portable sandbox (Windows Sandbox when available):
.\Start-PortableSandbox.bat
.\scripts\windows\Status.ps1
```

## Requirements

- **Windows 10/11 x64** (portable zip) or Linux with systemd (Debian / Ubuntu / Raspberry Pi OS 64-bit)
- Outbound HTTPS to:
  - `api2.cursor.sh`
  - `api2direct.cursor.sh`
  - `cloud-agent-artifacts.s3.us-east-1.amazonaws.com` (cloud uploads)
- A **personal** Cursor credential ([API Keys](https://cursor.com/dashboard/api) or `agent login`) for cloud mode
- A git checkout with a valid `origin` remote (cloud My Machines worker)

Service-account keys only work with `--pool` (Enterprise). Do not use them here.

## Quick start (Linux)

```bash
cd Japan-central/cursor-appliance
./scripts/setup.sh

# Option A — personal API key (good for headless boxes)
nano .env   # set CURSOR_API_KEY=...

# Option B — browser login (good for a desktop)
agent login

# Foreground smoke test
./scripts/start-worker.sh

# Or use the desktop control panel
./scripts/run-gui.sh
```

Leave that process running, then open [cursor.com/agents](https://cursor.com/agents) and select **cursor-appliance** in the environment / “Run on” dropdown.

### Desktop GUI

```bash
./scripts/run-gui.sh
```

The panel includes:

- **Settings** — appliance name, worker dirs/addrs, API key, idle timeout, auto-start, failover
- **Dark mode** — toggle in the top bar or Settings
- **Offline mode** — disconnects the cloud worker and starts the lightweight local worker
- **Local failover** — when the cloud worker stops (crash or Stop cloud), local worker takes over

Settings persist to `data/gui-settings.json` and sync into `.env` for the shell/systemd scripts.

### Local failover worker

When the My Machines cloud worker is down, `cursor-local-worker` stays on-box and:

- Serves `http://127.0.0.1:8734/healthz` and `/status`
- Accepts local jobs at `POST /v1/jobs` (`note`, `inspect`, allowlisted `shell`)
- Writes heartbeats under `data/local-worker-heartbeat.json`

```bash
./scripts/start-local-worker.sh
curl -fsS http://127.0.0.1:8734/status
# queue a note:
curl -fsS -X POST http://127.0.0.1:8734/v1/jobs \
  -H 'content-type: application/json' \
  -d '{"kind":"note","note":"cloud down; local takeover"}'
```

### Install as a boot service

```bash
sudo ./scripts/install_service.sh
./scripts/status.sh
journalctl -u cursor-appliance -f
```

Uninstall:

```bash
sudo ./scripts/uninstall_service.sh
```

## Trigger this machine from Slack / GitHub / Linear

Start the worker with `--name cursor-appliance` (the default), then target it:

```text
@Cursor worker=cursor-appliance fix the flaky test
@cursoragent worker=cursor-appliance fix the flaky test
```

Cursor only routes the job here when the named machine belongs to your user **and** the worker’s git remote matches the target repo.

## Config

Copy `.env.example` → `.env` (gitignored).

| Variable | Default | Meaning |
|---|---|---|
| `CURSOR_API_KEY` | _(empty)_ | Personal user API key |
| `CURSOR_APPLIANCE_NAME` | `cursor-appliance` | Name in the agents UI / `worker=` triggers |
| `CURSOR_APPLIANCE_WORKER_DIR` | repo root | Git checkout exposed to agents |
| `CURSOR_APPLIANCE_MANAGEMENT_ADDR` | `127.0.0.1:8733` | Cloud worker health bind |
| `CURSOR_APPLIANCE_LOCAL_ADDR` | `127.0.0.1:8734` | Local failover worker bind |
| `CURSOR_APPLIANCE_AUTH_TOKEN_FILE` | _(empty)_ | Optional rotating token file |
| `CURSOR_APPLIANCE_IDLE_RELEASE_TIMEOUT` | `0` | Idle auto-exit seconds (`0` = stay online) |
| `CURSOR_APPLIANCE_OFFLINE` | `0` | `1` blocks cloud start; local worker takes over |
| `CURSOR_APPLIANCE_LOCAL_FAILOVER` | `1` | Auto-start local worker when cloud stops |
| `CURSOR_APPLIANCE_DEBUG` | `0` | Set `1` for worker debug diagnostics |

Never commit API keys. Prefer `chmod 600 .env` (the service installer enforces this).

## Networking notes

- Workers dial **out** only. No public IP, inbound port, or VPN tunnel is required for Cursor itself.
- Keep `/healthz` on localhost unless you deliberately put it behind Tailscale.
- Proxy support: set `HTTPS_PROXY` / `https_proxy` in `.env`.

## Troubleshooting

```bash
# Preflight (auth, privacy routing, repo labels)
agent worker debug

# Verbose appliance start
CURSOR_APPLIANCE_DEBUG=1 ./scripts/start-worker.sh

# Service logs
journalctl -u cursor-appliance -f
./scripts/status.sh
```

| Symptom | Check |
|---|---|
| Machine missing from agents UI | Worker process up? Same Cursor user? Repo remote matches? |
| Auth errors | Personal API key / `agent login` (not service-account) |
| Artifact uploads fail | Allow `cloud-agent-artifacts.s3.us-east-1.amazonaws.com` |
| Session won't start | Allow `api2.cursor.sh` and `api2direct.cursor.sh` |

## Safety

- The worker can run shell commands and edit files in the checkout as the service user.
- Run it under a normal user account (installer refuses root).
- Treat the appliance host like a trusted developer machine.

## License

MIT
