"""Inference wrapper for BorikenGPT + deterministic reconstruction engine."""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Any

import torch

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from train.train_lm import BorikenGPT, CharTokenizer, GPTConfig  # noqa: E402

DEFAULT_MODEL = ROOT / "models" / "boriken-gpt.pt"


class BorikenLLM:
    def __init__(self, model_path: Path | None = None) -> None:
        self.model_path = model_path or DEFAULT_MODEL
        self.model: BorikenGPT | None = None
        self.tokenizer: CharTokenizer | None = None
        self.device = "cuda" if torch.cuda.is_available() else "cpu"
        self.meta: dict[str, Any] = {}
        if self.model_path.exists():
            self.load(self.model_path)

    @property
    def ready(self) -> bool:
        return self.model is not None and self.tokenizer is not None

    def load(self, path: Path) -> None:
        payload = torch.load(path, map_location=self.device, weights_only=False)
        config = GPTConfig(**payload["config"])
        tok = CharTokenizer("")
        tok.stoi = {k: int(v) for k, v in payload["tokenizer"]["stoi"].items()}
        tok.itos = {int(v): k for k, v in tok.stoi.items()}
        model = BorikenGPT(config)
        model.load_state_dict(payload["model"])
        model.to(self.device)
        model.eval()
        self.model = model
        self.tokenizer = tok
        self.meta = payload.get("meta", {})

    @torch.no_grad()
    def complete(self, prompt: str, max_new_tokens: int = 64, temperature: float = 0.7) -> str:
        if not self.ready:
            return ""
        assert self.model is not None and self.tokenizer is not None
        ids = self.tokenizer.encode(prompt)
        if not ids:
            ids = self.tokenizer.encode(" ")
        idx = torch.tensor([ids], dtype=torch.long, device=self.device)
        out = self.model.generate(idx, max_new_tokens=max_new_tokens, temperature=temperature)
        return self.tokenizer.decode(out[0].tolist())

    def info(self) -> dict[str, Any]:
        return {
            "ready": self.ready,
            "model_path": str(self.model_path),
            "device": self.device,
            "meta": self.meta,
        }
