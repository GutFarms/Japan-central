from __future__ import annotations

from pi_invest.data import mean, returns, stdev
from pi_invest.models import Bar, Quote, SignalScore, Side, TradeIntent


def _clamp(x: float, lo: float = 0.0, hi: float = 1.0) -> float:
    return max(lo, min(hi, x))


def score_symbol(quote: Quote, history: list[Bar]) -> SignalScore:
    closes = [b.close for b in history]
    rets = returns(closes)
    mom_20 = 0.0
    if len(closes) >= 21:
        mom_20 = (closes[-1] / closes[-21]) - 1.0
    elif len(closes) >= 2:
        mom_20 = (closes[-1] / closes[0]) - 1.0

    vol = stdev(rets)
    avg_ret = mean(rets)

    # Trend: fraction of recent closes above SMA20
    sma_n = min(20, len(closes))
    sma = mean(closes[-sma_n:]) if sma_n else closes[-1]
    trend = _clamp(0.5 + (closes[-1] - sma) / sma * 5) if sma else 0.5

    # Dividend / income proxy (known ETFs carry yield; growth names score lower)
    dy = quote.dividend_yield or 0.0
    yield_score = _clamp(dy / 0.06)  # 6% yield => 1.0

    momentum = _clamp(0.5 + mom_20 * 4)  # ~12.5% move => 1.0
    volatility_penalty = _clamp(vol / 0.03)  # daily 3% vol => full penalty

    # Expected income proxy blends yield with positive drift, penalized by vol
    expected_income_proxy = (yield_score * 0.55) + (momentum * 0.30) + (trend * 0.25)
    expected_income_proxy -= volatility_penalty * 0.25
    expected_income_proxy = _clamp(expected_income_proxy)

    composite = (
        0.35 * yield_score
        + 0.30 * momentum
        + 0.20 * trend
        + 0.15 * (1.0 - volatility_penalty)
    )
    composite = _clamp(composite)

    notes: list[str] = []
    if dy >= 0.03:
        notes.append(f"yield~{dy:.1%}")
    if mom_20 > 0.03:
        notes.append(f"mom+{mom_20:.1%}")
    elif mom_20 < -0.03:
        notes.append(f"mom{mom_20:.1%}")
    if vol > 0.02:
        notes.append("elevated vol")
    if avg_ret > 0:
        notes.append("positive drift")

    return SignalScore(
        symbol=quote.symbol.upper(),
        momentum=round(momentum, 4),
        yield_score=round(yield_score, 4),
        trend=round(trend, 4),
        volatility_penalty=round(volatility_penalty, 4),
        composite=round(composite, 4),
        expected_income_proxy=round(expected_income_proxy, 4),
        notes=notes,
    )


def heuristic_intents(
    scores: list[SignalScore],
    top_n: int = 4,
) -> list[TradeIntent]:
    """Allocate more weight to higher expected-income scores."""
    ranked = sorted(scores, key=lambda s: s.expected_income_proxy, reverse=True)
    buys = [s for s in ranked if s.expected_income_proxy >= 0.45][:top_n]
    if not buys:
        return []

    # Softmax-ish weights from scores
    exps = [2.71828 ** (s.expected_income_proxy * 3) for s in buys]
    total = sum(exps) or 1.0
    intents: list[TradeIntent] = []
    for s, e in zip(buys, exps):
        weight = 0.08 + 0.10 * (e / total)  # ~8–18% each, risk gate caps later
        intents.append(
            TradeIntent(
                symbol=s.symbol,
                side=Side.BUY,
                target_weight=round(min(weight, 0.15), 4),
                confidence=round(s.composite, 4),
                rationale=(
                    f"heuristic income proxy={s.expected_income_proxy:.2f}; "
                    + ", ".join(s.notes or ["balanced"])
                ),
                source="heuristic",
            )
        )

    # Suggest trimming weak holdings later in orchestrator via sells for bottom scores
    weak = [s for s in ranked if s.expected_income_proxy < 0.35][-3:]
    for s in weak:
        intents.append(
            TradeIntent(
                symbol=s.symbol,
                side=Side.SELL,
                target_weight=0.0,
                confidence=round(1.0 - s.composite, 4),
                rationale=f"weak income proxy={s.expected_income_proxy:.2f}",
                source="heuristic",
            )
        )
    return intents
