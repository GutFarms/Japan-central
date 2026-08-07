from __future__ import annotations

from pathlib import Path

from pi_invest.agent import InvestAgent
from pi_invest.broker import build_broker
from pi_invest.config import AppConfig, EnvSettings, load_config, load_env
from pi_invest.data import build_market_data
from pi_invest.storage.db import Database


def project_root() -> Path:
    # src/pi_invest/factory.py -> pi-invest-agent/
    return Path(__file__).resolve().parents[2]


def build_agent(
    config_path: str | None = None,
    force_simulator: bool = False,
) -> tuple[InvestAgent, AppConfig, EnvSettings, Database]:
    root = project_root()
    env = load_env()

    # Prefer paths relative to project root when running as a service
    cfg_path = Path(config_path or env.pi_invest_config)
    if not cfg_path.is_absolute():
        cfg_path = root / cfg_path
    if not cfg_path.exists():
        example = root / "config" / "config.example.yaml"
        cfg_path = example

    cfg = load_config(cfg_path)

    db_path = Path(env.pi_invest_db)
    if not db_path.is_absolute():
        db_path = root / db_path
    db = Database(db_path)

    provider = "simulator" if force_simulator else cfg.market.provider
    market = build_market_data(
        provider=provider,
        allow_simulator_fallback=cfg.agent.allow_simulator_fallback,
    )
    broker = build_broker(cfg, env, db)
    agent = InvestAgent(cfg, env, market, broker, db)
    return agent, cfg, env, db
