# Pi Invest Agent

Autonomous income-seeking investment agent designed for **Raspberry Pi 5**.

It scores tickers for expected income (dividends + capital appreciation signals), asks an optional local/cloud LLM to refine the plan, then routes every order through hard risk limits. **Paper trading is the default.** Live brokerage requires an explicit unlock.

## What it does

1. Pulls market snapshots (Yahoo Finance public quotes; works offline with a built-in simulator).
2. Scores candidates on momentum, dividend yield proxy, volatility risk, and trend.
3. Optionally asks **Ollama** (recommended on Pi 5) or OpenAI to pick/weight ideas.
4. Risk gate enforces max position size, daily loss halt, cash reserve, and allowlist.
5. Executes via local paper ledger or Alpaca (paper/live).
6. Exposes a tiny FastAPI status dashboard + CLI.

## Safety defaults

| Control | Default |
|---|---|
| Trading mode | `paper` |
| Live unlock | Off (`ALLOW_LIVE_TRADING=false`) |
| Max position | 15% of equity |
| Max daily loss | 3% of equity |
| Cash reserve | 10% kept uninvested |
| Universe | Configurable allowlist only |

> This is software for experimentation. It is **not** financial advice. Automated trading can lose money. Start in paper mode and only unlock live capital if you understand the risks.

## Raspberry Pi 5 quick start

```bash
# On the Pi (Bookworm / 64-bit recommended)
sudo apt update
sudo apt install -y python3-venv python3-pip git

git clone <your-repo-url>
cd Japan-central/pi-invest-agent

python3 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"

cp config/config.example.yaml config/config.yaml
cp .env.example .env

# Dry-run one decision cycle (simulator, no network required)
pi-invest once --dry-run

# Continuous paper loop
pi-invest run

# Status dashboard (http://<pi-ip>:8787)
pi-invest dashboard
```

Optional local LLM (strong on Pi 5 with 8GB+ RAM):

```bash
curl -fsSL https://ollama.com/install.sh | sh
ollama pull llama3.2:3b
# set llm.provider: ollama in config/config.yaml
```

Install as a systemd service:

```bash
sudo ./scripts/install_service.sh
sudo systemctl enable --now pi-invest
journalctl -u pi-invest -f
```

## CLI

```bash
pi-invest once          # single research + trade cycle
pi-invest run           # scheduled loop
pi-invest status        # portfolio + recent decisions
pi-invest dashboard     # web UI on :8787
pi-invest reset-paper   # wipe paper ledger (keeps config)
```

## Config

Edit `config/config.yaml` (copied from the example):

- `universe` — tickers the agent may touch
- `risk.*` — hard caps
- `broker.backend` — `paper` or `alpaca`
- `llm.provider` — `none` | `ollama` | `openai`
- `schedule.interval_minutes` — how often to reassess

Secrets go in `.env` (never commit):

```
ALPACA_API_KEY=
ALPACA_SECRET_KEY=
OPENAI_API_KEY=
ALLOW_LIVE_TRADING=false
```

## Project layout

```
pi-invest-agent/
  src/pi_invest/
    agent/       # scoring brain + risk gate + orchestrator
    broker/      # paper + Alpaca adapters
    data/        # market snapshots
    storage/     # SQLite trade/decision log
    web/         # FastAPI dashboard
  config/
  systemd/
  scripts/
  tests/
```

## License

MIT — use at your own risk.
