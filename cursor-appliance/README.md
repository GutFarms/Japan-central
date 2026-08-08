# Cursor Appliance

**Version 0.1.0** — a lightweight, dedicated **local** [My Machines](https://cursor.com/docs/cloud-agent/self-hosted-guides/my-machines) worker for this repo.

Cursor keeps planning/inference in the cloud. Tool calls (shell, edits, browser, local MCP) run on **this machine**. No inbound ports or VPN required.

Use this when you want a always-on local box (laptop, mini PC, or Raspberry Pi) that shows up as `cursor-appliance` on [cursor.com/agents](https://cursor.com/agents).

## What you get

| Piece | Role |
|---|---|
| `scripts/setup.sh` | Installs the Cursor Agent CLI and prepares config |
| `scripts/start-worker.sh` | Starts a named My Machines worker for this checkout |
| `systemd/cursor-appliance.service` | Keeps the worker alive across reboots |
| `scripts/status.sh` | Local health / process check |
| Local `:8733` | Optional `/healthz` `/readyz` `/metrics` |

This is **not** an Enterprise Self-Hosted Pool and **not** a Cursor-managed cloud VM. It is a personal My Machines appliance.

## Requirements

- Linux with systemd (Debian / Ubuntu / Raspberry Pi OS 64-bit work well)
- Outbound HTTPS to:
  - `api2.cursor.sh`
  - `api2direct.cursor.sh`
  - `cloud-agent-artifacts.s3.us-east-1.amazonaws.com` (artifacts uploads)
- A **personal** Cursor credential ([API Keys](https://cursor.com/dashboard/api) or `agent login`)
- This repository checked out with a valid `origin` remote

Service-account keys only work with `--pool` (Enterprise). Do not use them here.

## Quick start

```bash
cd Japan-central/cursor-appliance
./scripts/setup.sh

# Option A — personal API key (good for headless boxes)
nano .env   # set CURSOR_API_KEY=...

# Option B — browser login (good for a desktop)
agent login

# Foreground smoke test
./scripts/start-worker.sh
```

Leave that process running, then open [cursor.com/agents](https://cursor.com/agents) and select **cursor-appliance** in the environment / “Run on” dropdown.

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
| `CURSOR_APPLIANCE_MANAGEMENT_ADDR` | `127.0.0.1:8733` | Local health bind |
| `CURSOR_APPLIANCE_AUTH_TOKEN_FILE` | _(empty)_ | Optional rotating token file |
| `CURSOR_APPLIANCE_IDLE_RELEASE_TIMEOUT` | `0` | Idle auto-exit seconds (`0` = stay online) |
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
