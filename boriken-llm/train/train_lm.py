"""Train a tiny character-level GPT for Boriken language modeling.

Designed to run on CPU and export weights for the iOS-facing API.
This is a domain language model for reconstruction/tutoring scaffolding—
not a claim of classical Taíno fluency.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
import time
from dataclasses import asdict, dataclass
from pathlib import Path

import torch
import torch.nn as nn
from torch.nn import functional as F

ROOT = Path(__file__).resolve().parents[1]


@dataclass
class GPTConfig:
    vocab_size: int
    block_size: int = 128
    n_layer: int = 4
    n_head: int = 4
    n_embd: int = 128
    dropout: float = 0.1


class CausalSelfAttention(nn.Module):
    def __init__(self, config: GPTConfig) -> None:
        super().__init__()
        assert config.n_embd % config.n_head == 0
        self.n_head = config.n_head
        self.n_embd = config.n_embd
        self.key = nn.Linear(config.n_embd, config.n_embd)
        self.query = nn.Linear(config.n_embd, config.n_embd)
        self.value = nn.Linear(config.n_embd, config.n_embd)
        self.proj = nn.Linear(config.n_embd, config.n_embd)
        self.attn_drop = nn.Dropout(config.dropout)
        self.resid_drop = nn.Dropout(config.dropout)
        mask = torch.tril(torch.ones(config.block_size, config.block_size))
        self.register_buffer("mask", mask.view(1, 1, config.block_size, config.block_size))

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        B, T, C = x.size()
        k = self.key(x).view(B, T, self.n_head, C // self.n_head).transpose(1, 2)
        q = self.query(x).view(B, T, self.n_head, C // self.n_head).transpose(1, 2)
        v = self.value(x).view(B, T, self.n_head, C // self.n_head).transpose(1, 2)
        att = (q @ k.transpose(-2, -1)) * (1.0 / math.sqrt(k.size(-1)))
        att = att.masked_fill(self.mask[:, :, :T, :T] == 0, float("-inf"))
        att = F.softmax(att, dim=-1)
        att = self.attn_drop(att)
        y = att @ v
        y = y.transpose(1, 2).contiguous().view(B, T, C)
        return self.resid_drop(self.proj(y))


class Block(nn.Module):
    def __init__(self, config: GPTConfig) -> None:
        super().__init__()
        self.ln1 = nn.LayerNorm(config.n_embd)
        self.attn = CausalSelfAttention(config)
        self.ln2 = nn.LayerNorm(config.n_embd)
        self.mlp = nn.Sequential(
            nn.Linear(config.n_embd, 4 * config.n_embd),
            nn.GELU(),
            nn.Linear(4 * config.n_embd, config.n_embd),
            nn.Dropout(config.dropout),
        )

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        x = x + self.attn(self.ln1(x))
        x = x + self.mlp(self.ln2(x))
        return x


class BorikenGPT(nn.Module):
    def __init__(self, config: GPTConfig) -> None:
        super().__init__()
        self.config = config
        self.tok_emb = nn.Embedding(config.vocab_size, config.n_embd)
        self.pos_emb = nn.Embedding(config.block_size, config.n_embd)
        self.drop = nn.Dropout(config.dropout)
        self.blocks = nn.ModuleList([Block(config) for _ in range(config.n_layer)])
        self.ln_f = nn.LayerNorm(config.n_embd)
        self.head = nn.Linear(config.n_embd, config.vocab_size, bias=False)
        self.apply(self._init_weights)

    def _init_weights(self, module: nn.Module) -> None:
        if isinstance(module, (nn.Linear, nn.Embedding)):
            nn.init.normal_(module.weight, mean=0.0, std=0.02)
            if isinstance(module, nn.Linear) and module.bias is not None:
                nn.init.zeros_(module.bias)

    def forward(self, idx: torch.Tensor, targets: torch.Tensor | None = None):
        B, T = idx.size()
        assert T <= self.config.block_size
        pos = torch.arange(0, T, device=idx.device).unsqueeze(0)
        x = self.drop(self.tok_emb(idx) + self.pos_emb(pos))
        for block in self.blocks:
            x = block(x)
        x = self.ln_f(x)
        logits = self.head(x)
        loss = None
        if targets is not None:
            loss = F.cross_entropy(logits.view(-1, logits.size(-1)), targets.view(-1))
        return logits, loss

    @torch.no_grad()
    def generate(self, idx: torch.Tensor, max_new_tokens: int, temperature: float = 0.8) -> torch.Tensor:
        for _ in range(max_new_tokens):
            idx_cond = idx[:, -self.config.block_size :]
            logits, _ = self(idx_cond)
            logits = logits[:, -1, :] / max(temperature, 1e-6)
            probs = F.softmax(logits, dim=-1)
            next_id = torch.multinomial(probs, num_samples=1)
            idx = torch.cat([idx, next_id], dim=1)
        return idx


class CharTokenizer:
    def __init__(self, text: str) -> None:
        chars = sorted(set(text))
        self.stoi = {ch: i for i, ch in enumerate(chars)}
        self.itos = {i: ch for ch, i in self.stoi.items()}

    @property
    def vocab_size(self) -> int:
        return len(self.stoi)

    def encode(self, s: str) -> list[int]:
        return [self.stoi[c] for c in s if c in self.stoi]

    def decode(self, ids: list[int]) -> str:
        return "".join(self.itos[i] for i in ids)

    def to_dict(self) -> dict:
        return {"stoi": self.stoi}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--steps", type=int, default=800)
    parser.add_argument("--batch-size", type=int, default=32)
    parser.add_argument("--block-size", type=int, default=128)
    parser.add_argument("--lr", type=float, default=3e-4)
    parser.add_argument("--out", type=Path, default=ROOT / "models" / "boriken-gpt.pt")
    args = parser.parse_args()

    data_path = ROOT / "corpus" / "training" / "lm_corpus.txt"
    if not data_path.exists():
        from build_dataset import build_lm_text, build_instruct_jsonl

        build_instruct_jsonl(ROOT / "corpus" / "training" / "instruct.jsonl")
        build_lm_text(data_path)

    text = data_path.read_text(encoding="utf-8")
    # Oversample to give the tiny model enough tokens
    text = (text + "\n") * 20
    tokenizer = CharTokenizer(text)
    data = torch.tensor(tokenizer.encode(text), dtype=torch.long)
    n = int(0.9 * len(data))
    train_data = data[:n]
    val_data = data[n:]

    config = GPTConfig(vocab_size=tokenizer.vocab_size, block_size=args.block_size)
    device = "cuda" if torch.cuda.is_available() else "cpu"
    model = BorikenGPT(config).to(device)
    optimizer = torch.optim.AdamW(model.parameters(), lr=args.lr)

    def get_batch(split: str):
        source = train_data if split == "train" else val_data
        ix = torch.randint(len(source) - args.block_size - 1, (args.batch_size,))
        x = torch.stack([source[i : i + args.block_size] for i in ix])
        y = torch.stack([source[i + 1 : i + 1 + args.block_size] for i in ix])
        return x.to(device), y.to(device)

    print(f"Device={device} vocab={tokenizer.vocab_size} params={sum(p.numel() for p in model.parameters()):,}")
    t0 = time.time()
    model.train()
    for step in range(1, args.steps + 1):
        xb, yb = get_batch("train")
        _, loss = model(xb, yb)
        optimizer.zero_grad(set_to_none=True)
        loss.backward()
        optimizer.step()
        if step % 100 == 0 or step == 1:
            model.eval()
            with torch.no_grad():
                _, vloss = model(*get_batch("val"))
            model.train()
            print(f"step {step:4d} train_loss={loss.item():.4f} val_loss={vloss.item():.4f}")

    args.out.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "config": asdict(config),
        "tokenizer": tokenizer.to_dict(),
        "model": model.state_dict(),
        "meta": {
            "name": "BorikenGPT",
            "version": "0.1.0",
            "trained_steps": args.steps,
            "device": device,
            "seconds": round(time.time() - t0, 2),
            "purpose": "Character LM over Boriken reconstruction corpus for iOS app scaffolding",
        },
    }
    torch.save(payload, args.out)
    meta_path = args.out.with_suffix(".json")
    meta_path.write_text(
        json.dumps({k: v for k, v in payload.items() if k != "model"}, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    print(f"Saved model to {args.out}")

    # Smoke generation
    model.eval()
    prompt = "Taíno "
    ids = torch.tensor([tokenizer.encode(prompt)], dtype=torch.long, device=device)
    out = model.generate(ids, max_new_tokens=40, temperature=0.7)[0].tolist()
    print("sample:", repr(tokenizer.decode(out)))


if __name__ == "__main__":
    main()
