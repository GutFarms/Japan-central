"""FastAPI server exposing BorikenLLM to the iOS app."""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Any, Literal

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel, Field

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from engine.corpus import load_corpus  # noqa: E402
from engine.llm import BorikenLLM  # noqa: E402
from engine.reconstruct import ReconstructionEngine  # noqa: E402
from engine.tutor import TutorEngine  # noqa: E402

app = FastAPI(
    title="BorikenLLM API",
    description="Language reconstruction & tutoring API for the Boriken iOS app",
    version="0.1.0",
)
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

corpus = load_corpus()
recon = ReconstructionEngine(corpus)
tutor = TutorEngine(corpus)
llm = BorikenLLM()


class TranslateRequest(BaseModel):
    text: str = Field(..., min_length=1, max_length=500)
    source_lang: Literal["auto", "en", "es", "boriken"] = "auto"


class ReconstructRequest(BaseModel):
    concept: str = Field(..., min_length=1, max_length=200)


class ChatRequest(BaseModel):
    message: str = Field(..., min_length=1, max_length=1000)


class CompleteRequest(BaseModel):
    prompt: str = Field(..., min_length=1, max_length=400)
    max_new_tokens: int = Field(48, ge=1, le=200)
    temperature: float = Field(0.7, ge=0.1, le=1.5)


class LessonRequest(BaseModel):
    skill: str | None = None


@app.get("/health")
def health() -> dict[str, Any]:
    return {
        "status": "ok",
        "language": "Boriken (Taíno-Borikenaíki)",
        "lexicon_size": len(corpus.entries),
        "llm": llm.info(),
        "version": "0.1.0",
    }


@app.get("/v1/lexicon")
def lexicon(q: str | None = None, tag: str | None = None, limit: int = 50) -> dict[str, Any]:
    if q:
        items = corpus.lookup(q)[:limit]
    elif tag:
        items = corpus.by_tag(tag)[:limit]
    else:
        items = corpus.entries[:limit]
    return {"count": len(items), "items": [i.to_dict() for i in items]}


@app.get("/v1/grammar")
def grammar() -> dict[str, Any]:
    return corpus.grammar


@app.get("/v1/sentences")
def sentences() -> dict[str, Any]:
    return corpus.sentences


@app.post("/v1/translate")
def translate(req: TranslateRequest) -> dict[str, Any]:
    result = recon.translate(req.text, source_lang=req.source_lang)
    return {"ok": True, "result": result.to_dict()}


@app.post("/v1/reconstruct")
def reconstruct(req: ReconstructRequest) -> dict[str, Any]:
    result = recon.reconstruct_concept(req.concept)
    return {"ok": True, "result": result.to_dict()}


@app.get("/v1/explain/{term}")
def explain(term: str) -> dict[str, Any]:
    return recon.explain(term)


@app.post("/v1/lesson")
def lesson(req: LessonRequest) -> dict[str, Any]:
    return tutor.lesson(req.skill)


@app.get("/v1/quiz")
def quiz(n: int = 5) -> dict[str, Any]:
    return {"cards": tutor.quiz(n)}


@app.post("/v1/chat")
def chat(req: ChatRequest) -> dict[str, Any]:
    return tutor.chat(req.message)


@app.post("/v1/complete")
def complete(req: CompleteRequest) -> dict[str, Any]:
    if not llm.ready:
        raise HTTPException(
            status_code=503,
            detail="Local BorikenGPT weights not found. Run: python train/train_lm.py",
        )
    text = llm.complete(req.prompt, max_new_tokens=req.max_new_tokens, temperature=req.temperature)
    return {"ok": True, "completion": text, "model": llm.info()}


@app.get("/v1/system-prompt")
def system_prompt() -> dict[str, str]:
    path = ROOT / "prompts" / "system.md"
    return {"prompt": path.read_text(encoding="utf-8")}


def main() -> None:
    import uvicorn

    uvicorn.run("api.server:app", host="0.0.0.0", port=8080, reload=False)


if __name__ == "__main__":
    main()
