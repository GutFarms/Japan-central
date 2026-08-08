# Local-only AI (no cloud)

Pi Invest OS is designed to run the income agent **entirely on the Raspberry Pi** after first boot.

## Stack

| Layer | On-device choice |
|---|---|
| LLM | Ollama (`127.0.0.1:11434`), default model `llama3.2:3b` |
| Market data | Simulator (offline). Optional: Yahoo with simulator fallback |
| Broker | Paper ledger |
| Wallet | Paper ledger |
| Alerts | Disabled (ntfy is cloud) |
| Updates | GitHub auto-update **off** |

`PI_INVEST_LOCAL_ONLY=true` forces this stack in the agent even if config still mentions OpenAI.

## First boot

Needs network **once** to:

1. `apt` install desktop + agent deps  
2. Install Ollama  
3. `ollama pull` the model  

Later cycles do not call OpenAI, Alpaca, Coinbase, or ntfy.

## Existing Pi

```bash
curl -fsSL https://raw.githubusercontent.com/GutFarms/Japan-central/cursor/pi-invest-os-0b6b/pi-invest-os/scripts/enable-local-ai-on-pi.sh | bash
```

## Verify

```bash
systemctl is-active ollama pi-invest pi-invest-dashboard
ollama list
cd /opt/pi-invest-agent && . .venv/bin/activate && pi-invest status
```

You should see `Local-only AI` with `ollama` and `simulator` / `paper`.

## Optional: live public quotes

Keep the LLM on Ollama, but allow Yahoo Finance quotes:

```bash
# in /opt/pi-invest-agent/.env
PI_INVEST_LOCAL_ONLY=true
PI_INVEST_ALLOW_ONLINE_QUOTES=true
```

```yaml
# config.yaml
market:
  provider: yahoo
```

Yahoo is public market data, not a cloud LLM. Offline fallback still applies when configured.

## Re-enable GitHub updates (optional)

```bash
# in /opt/pi-invest-agent/.env
PI_INVEST_AUTO_UPDATE=true
# and set PI_INVEST_LOCAL_ONLY=false only if you also want cloud APIs
sudo systemctl enable --now pi-invest-update.timer
```
