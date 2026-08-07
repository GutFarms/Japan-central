from __future__ import annotations

from pathlib import Path
from typing import Literal

import yaml
from pydantic import BaseModel, Field
from pydantic_settings import BaseSettings, SettingsConfigDict


class RiskConfig(BaseModel):
    max_position_pct: float = 0.15
    max_daily_loss_pct: float = 0.03
    cash_reserve_pct: float = 0.10
    max_open_positions: int = 8
    min_confidence: float = 0.55
    min_trade_notional: float = 25.0


class BrokerConfig(BaseModel):
    backend: Literal["paper", "alpaca"] = "paper"
    starting_cash: float = 10_000.0


class MarketConfig(BaseModel):
    lookback_days: int = 60
    provider: Literal["yahoo", "simulator"] = "yahoo"


class LlmConfig(BaseModel):
    provider: Literal["none", "ollama", "openai"] = "none"
    temperature: float = 0.2
    timeout_seconds: int = 45


class ScheduleConfig(BaseModel):
    interval_minutes: int = 60
    market_tz: str = "America/New_York"
    prefer_market_hours: bool = True


class DashboardConfig(BaseModel):
    host: str = "0.0.0.0"
    port: int = 8787


class AgentConfig(BaseModel):
    name: str = "pi-income-agent"
    mode: Literal["paper", "live"] = "paper"
    allow_simulator_fallback: bool = True


class AppConfig(BaseModel):
    agent: AgentConfig = Field(default_factory=AgentConfig)
    universe: list[str] = Field(
        default_factory=lambda: ["SCHD", "VYM", "JEPI", "QQQ", "SPY", "BND"]
    )
    risk: RiskConfig = Field(default_factory=RiskConfig)
    broker: BrokerConfig = Field(default_factory=BrokerConfig)
    market: MarketConfig = Field(default_factory=MarketConfig)
    llm: LlmConfig = Field(default_factory=LlmConfig)
    schedule: ScheduleConfig = Field(default_factory=ScheduleConfig)
    dashboard: DashboardConfig = Field(default_factory=DashboardConfig)


class EnvSettings(BaseSettings):
    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        extra="ignore",
    )

    alpaca_api_key: str = ""
    alpaca_secret_key: str = ""
    alpaca_base_url: str = "https://paper-api.alpaca.markets"
    openai_api_key: str = ""
    openai_model: str = "gpt-4o-mini"
    ollama_base_url: str = "http://127.0.0.1:11434"
    ollama_model: str = "llama3.2:3b"
    allow_live_trading: bool = False
    pi_invest_config: str = "config/config.yaml"
    pi_invest_db: str = "data/pi_invest.db"


def load_config(path: str | Path | None = None) -> AppConfig:
    env = EnvSettings()
    cfg_path = Path(path or env.pi_invest_config)
    if not cfg_path.exists():
        example = cfg_path.parent / "config.example.yaml"
        if example.exists():
            cfg_path = example
        else:
            return AppConfig()
    with cfg_path.open("r", encoding="utf-8") as fh:
        raw = yaml.safe_load(fh) or {}
    return AppConfig.model_validate(raw)


def load_env() -> EnvSettings:
    return EnvSettings()
