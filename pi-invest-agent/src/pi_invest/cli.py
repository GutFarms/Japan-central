from __future__ import annotations

import time
from typing import Optional

import typer
from rich.console import Console
from rich.table import Table

from pi_invest.factory import build_agent

app = typer.Typer(
    name="pi-invest",
    help="Raspberry Pi 5 autonomous income investment agent",
    add_completion=False,
)
console = Console()


@app.command()
def once(
    dry_run: bool = typer.Option(False, help="Score and plan without placing orders"),
    simulator: bool = typer.Option(False, help="Force offline simulator quotes"),
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Run a single research + trade cycle."""
    agent, cfg, _env, _db = build_agent(config_path=config, force_simulator=simulator)
    console.print(
        f"[bold]{cfg.agent.name}[/bold] mode={cfg.agent.mode} backend={cfg.broker.backend}"
    )
    decision = agent.run_cycle(dry_run=dry_run)
    _print_decision(decision)


@app.command()
def run(
    simulator: bool = typer.Option(False, help="Force offline simulator quotes"),
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Run continuously on the configured interval."""
    agent, cfg, _env, _db = build_agent(config_path=config, force_simulator=simulator)
    interval = max(1, cfg.schedule.interval_minutes) * 60
    console.print(
        f"Starting loop every {cfg.schedule.interval_minutes}m "
        f"(mode={cfg.agent.mode}, ctrl+c to stop)"
    )
    while True:
        try:
            decision = agent.run_cycle(dry_run=False)
            _print_decision(decision)
        except Exception as exc:  # noqa: BLE001
            console.print(f"[red]cycle error:[/red] {exc}")
        time.sleep(interval)


@app.command()
def status(
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Show portfolio and recent decisions."""
    agent, cfg, _env, db = build_agent(config_path=config, force_simulator=True)
    # Use last known marks from simulator for display if needed
    marks = {}
    for sym in cfg.universe:
        try:
            marks[sym] = agent.market.get_quote(sym).price
        except Exception:  # noqa: BLE001
            pass
    acct = agent.broker.get_account(marks)

    console.print(
        f"[bold]Equity[/bold] ${acct.equity:,.2f}  "
        f"[bold]Cash[/bold] ${acct.cash:,.2f}  "
        f"[bold]Day PnL[/bold] ${acct.day_pnl:,.2f} ({acct.day_pnl_pct:.2%})"
    )
    table = Table(title="Positions")
    table.add_column("Symbol")
    table.add_column("Qty", justify="right")
    table.add_column("Avg", justify="right")
    table.add_column("Mark", justify="right")
    table.add_column("Value", justify="right")
    table.add_column("uPnL", justify="right")
    for p in acct.positions:
        table.add_row(
            p.symbol,
            f"{p.qty:.4f}",
            f"{p.avg_cost:.2f}",
            f"{p.market_price:.2f}",
            f"{p.market_value:.2f}",
            f"{p.unrealized_pnl:.2f}",
        )
    console.print(table)

    decisions = db.recent_decisions(5)
    console.print(f"\n[bold]Recent decisions[/bold]: {len(decisions)}")
    for d in decisions:
        fills = [o for o in d.get("orders", []) if o.get("ok")]
        console.print(
            f"  {d.get('timestamp')}  scores={len(d.get('scores', []))}  "
            f"fills={len(fills)}  source={d.get('meta', {}).get('data_source')}"
        )


@app.command("reset-paper")
def reset_paper(
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
    yes: bool = typer.Option(False, "--yes", help="Skip confirmation"),
) -> None:
    """Wipe the local paper ledger back to starting cash."""
    if not yes and not typer.confirm("Reset paper account?"):
        raise typer.Abort()
    agent, cfg, _env, _db = build_agent(config_path=config, force_simulator=True)
    agent.broker.reset()
    console.print(f"Paper account reset to ${cfg.broker.starting_cash:,.2f}")


@app.command()
def dashboard(
    config: Optional[str] = typer.Option(None, help="Path to config.yaml"),
) -> None:
    """Start the local status dashboard."""
    import uvicorn

    from pi_invest.web.app import create_app

    agent, cfg, env, db = build_agent(config_path=config)
    api = create_app(agent, cfg, db)
    console.print(
        f"Dashboard on http://{cfg.dashboard.host}:{cfg.dashboard.port}"
    )
    uvicorn.run(api, host=cfg.dashboard.host, port=cfg.dashboard.port, log_level="info")


def _print_decision(decision) -> None:
    console.print(f"\n[bold]Cycle[/bold] {decision.cycle_id[:8]}…")
    console.print(
        f"market_open={decision.market_open}  "
        f"data={decision.meta.get('data_source')}  "
        f"mode={decision.meta.get('mode')}"
    )
    score_table = Table(title="Income scores")
    score_table.add_column("Symbol")
    score_table.add_column("Income", justify="right")
    score_table.add_column("Composite", justify="right")
    score_table.add_column("Notes")
    for s in decision.scores[:8]:
        score_table.add_row(
            s.symbol,
            f"{s.expected_income_proxy:.2f}",
            f"{s.composite:.2f}",
            ", ".join(s.notes) or "—",
        )
    console.print(score_table)

    if decision.orders:
        for o in decision.orders:
            color = "green" if o.ok else "yellow"
            console.print(
                f"[{color}]order[/{color}] {o.side.value} {o.symbol} "
                f"qty={o.qty:.4f} @ {o.fill_price:.2f} — {o.message}"
            )
    for reason in decision.skipped_reasons:
        console.print(f"[dim]skip:[/dim] {reason}")

    if decision.account_after:
        a = decision.account_after
        console.print(
            f"[bold]Account[/bold] equity=${a.equity:,.2f} cash=${a.cash:,.2f} "
            f"positions={len(a.positions)}"
        )


if __name__ == "__main__":
    app()
