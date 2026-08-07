from __future__ import annotations

from fastapi import FastAPI
from fastapi.responses import HTMLResponse

from pi_invest.agent import InvestAgent
from pi_invest.config import AppConfig
from pi_invest.storage.db import Database

DASHBOARD_HTML = """<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Pi Invest Agent</title>
  <style>
    :root {
      --bg: #0f1410;
      --panel: #182018;
      --ink: #e7efe6;
      --muted: #8fa08c;
      --accent: #7cb87c;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: "IBM Plex Sans", "Segoe UI", sans-serif;
      background:
        radial-gradient(1200px 600px at 10% -10%, #1d3320 0%, transparent 55%),
        radial-gradient(900px 500px at 100% 0%, #243028 0%, transparent 50%),
        var(--bg);
      color: var(--ink);
      min-height: 100vh;
    }
    main { max-width: 920px; margin: 0 auto; padding: 2.5rem 1.25rem 4rem; }
    h1 {
      font-family: "IBM Plex Serif", Georgia, serif;
      font-weight: 600;
      font-size: clamp(1.8rem, 4vw, 2.6rem);
      margin: 0 0 0.35rem;
      letter-spacing: -0.02em;
    }
    .sub { color: var(--muted); margin-bottom: 2rem; }
    .grid { display: grid; gap: 1rem; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); }
    .stat {
      padding: 1rem 1.1rem;
      border: 1px solid #2a3a2c;
      background: color-mix(in srgb, var(--panel) 90%, transparent);
    }
    .stat .label { color: var(--muted); font-size: 0.8rem; text-transform: uppercase; letter-spacing: 0.06em; }
    .stat .value { font-size: 1.45rem; margin-top: 0.35rem; font-variant-numeric: tabular-nums; }
    h2 { margin: 2rem 0 0.75rem; font-size: 1.05rem; color: var(--accent); font-weight: 600; }
    table { width: 100%; border-collapse: collapse; font-size: 0.95rem; }
    th, td { text-align: left; padding: 0.55rem 0.4rem; border-bottom: 1px solid #243028; }
    th { color: var(--muted); font-weight: 500; font-size: 0.78rem; text-transform: uppercase; letter-spacing: 0.05em; }
    .muted { color: var(--muted); }
    .pill {
      display: inline-block;
      border: 1px solid #3a523c;
      padding: 0.15rem 0.55rem;
      font-size: 0.75rem;
      color: var(--accent);
    }
    button {
      margin-top: 1.5rem;
      background: var(--accent);
      color: #0c120e;
      border: 0;
      padding: 0.65rem 1rem;
      font-weight: 600;
      cursor: pointer;
    }
    button:hover { filter: brightness(1.08); }
    #err { color: #e08080; margin-top: 1rem; }
  </style>
</head>
<body>
  <main>
    <h1>Pi Invest</h1>
    <p class="sub">Income-seeking agent on Raspberry Pi 5 · <span id="mode" class="pill">…</span></p>
    <div class="grid">
      <div class="stat"><div class="label">Equity</div><div class="value" id="equity">—</div></div>
      <div class="stat"><div class="label">Cash</div><div class="value" id="cash">—</div></div>
      <div class="stat"><div class="label">Day PnL</div><div class="value" id="pnl">—</div></div>
      <div class="stat"><div class="label">Positions</div><div class="value" id="npos">—</div></div>
    </div>
    <h2>Holdings</h2>
    <table>
      <thead><tr><th>Symbol</th><th>Qty</th><th>Value</th><th>uPnL</th></tr></thead>
      <tbody id="positions"></tbody>
    </table>
    <h2>Latest income scores</h2>
    <table>
      <thead><tr><th>Symbol</th><th>Income</th><th>Composite</th><th>Notes</th></tr></thead>
      <tbody id="scores"></tbody>
    </table>
    <h2>Recent fills</h2>
    <table>
      <thead><tr><th>When</th><th>Side</th><th>Symbol</th><th>Qty</th><th>Price</th></tr></thead>
      <tbody id="orders"></tbody>
    </table>
    <button id="cycle">Run one cycle</button>
    <div id="err"></div>
  </main>
  <script>
    const fmt = (n) => Number(n).toLocaleString(undefined, {style:'currency', currency:'USD'});
    async function refresh() {
      const r = await fetch('/api/status');
      const d = await r.json();
      document.getElementById('mode').textContent = `${d.mode} / ${d.backend}`;
      document.getElementById('equity').textContent = fmt(d.account.equity);
      document.getElementById('cash').textContent = fmt(d.account.cash);
      document.getElementById('pnl').textContent = `${fmt(d.account.day_pnl)} (${(d.account.day_pnl_pct*100).toFixed(2)}%)`;
      document.getElementById('npos').textContent = d.account.positions.length;
      document.getElementById('positions').innerHTML = d.account.positions.map(p =>
        `<tr><td>${p.symbol}</td><td>${p.qty.toFixed(4)}</td><td>${fmt(p.market_value)}</td><td>${fmt(p.unrealized_pnl)}</td></tr>`
      ).join('') || '<tr><td colspan=4 class="muted">No positions</td></tr>';
      document.getElementById('scores').innerHTML = (d.latest_scores||[]).map(s =>
        `<tr><td>${s.symbol}</td><td>${s.expected_income_proxy.toFixed(2)}</td><td>${s.composite.toFixed(2)}</td><td class="muted">${(s.notes||[]).join(', ')}</td></tr>`
      ).join('') || '<tr><td colspan=4 class="muted">No cycle yet</td></tr>';
      document.getElementById('orders').innerHTML = (d.orders||[]).map(o =>
        `<tr><td class="muted">${o.created_at}</td><td>${o.side}</td><td>${o.symbol}</td><td>${Number(o.qty).toFixed(4)}</td><td>${fmt(o.fill_price)}</td></tr>`
      ).join('') || '<tr><td colspan=5 class="muted">No fills yet</td></tr>';
    }
    document.getElementById('cycle').onclick = async () => {
      document.getElementById('err').textContent = '';
      try {
        const r = await fetch('/api/cycle', {method:'POST'});
        if (!r.ok) throw new Error(await r.text());
        await refresh();
      } catch (e) {
        document.getElementById('err').textContent = String(e);
      }
    };
    refresh();
    setInterval(refresh, 15000);
  </script>
</body>
</html>
"""


def create_app(agent: InvestAgent, cfg: AppConfig, db: Database) -> FastAPI:
    app = FastAPI(title="Pi Invest Agent", version="0.1.0")

    @app.get("/", response_class=HTMLResponse)
    def home() -> str:
        return DASHBOARD_HTML

    @app.get("/api/status")
    def status() -> dict:
        marks = {}
        for sym in cfg.universe:
            try:
                marks[sym] = agent.market.get_quote(sym).price
            except Exception:  # noqa: BLE001
                pass
        account = agent.broker.get_account(marks)
        decisions = db.recent_decisions(1)
        latest_scores = decisions[0].get("scores", []) if decisions else []
        return {
            "mode": cfg.agent.mode,
            "backend": cfg.broker.backend,
            "agent": cfg.agent.name,
            "account": account.model_dump(),
            "latest_scores": latest_scores,
            "orders": db.recent_orders(15),
        }

    @app.post("/api/cycle")
    def cycle() -> dict:
        decision = agent.run_cycle(dry_run=False)
        return decision.model_dump(mode="json")

    @app.get("/api/health")
    def health() -> dict:
        return {"ok": True, "agent": cfg.agent.name}

    return app
