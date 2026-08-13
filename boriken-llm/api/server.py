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
from engine.fun import FunLearnEngine  # noqa: E402
from engine.llm import BorikenLLM  # noqa: E402
from engine.reconstruct import ReconstructionEngine  # noqa: E402
from engine.sentence import SentenceStructureEngine  # noqa: E402
from engine.tutor import TutorEngine  # noqa: E402

app = FastAPI(
    title="BorikenLLM API",
    description="Fun Boriken language reconstruction & learning API for iOS",
    version="0.3.1",
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
fun = FunLearnEngine(corpus)
sentences = SentenceStructureEngine(corpus)
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


class GradeRequest(BaseModel):
    mode: str = "practice"
    answer: str
    expected: str


class ProgressRequest(BaseModel):
    xp: int = 0
    streak: int = 0


class SentenceBuildRequest(BaseModel):
    pattern_id: str = Field(..., min_length=1)
    subject: str | None = None
    verb: str | None = None
    object: str | None = None
    noun: str | None = None
    quality: str | None = None
    place: str | None = None
    recipient: str | None = None
    predicate: str | None = None
    vocative: str | None = None


@app.get("/health")
def health() -> dict[str, Any]:
    return {
        "status": "ok",
        "language": "Boriken (Taíno-Borikenaíki)",
        "lexicon_size": len(corpus.entries),
        "has_definitions": bool(getattr(corpus.entries[0], "definition_en", "")),
        "sentence_patterns": len(corpus.grammar.get("sentence_patterns", [])),
        "llm": llm.info(),
        "version": "0.3.1",
        "play": "/v1/fun/menu",
        "sentence_structure": "/v1/sentences/structure",
        "accuracy": "/v1/accuracy",
    }


@app.get("/v1/accuracy")
def accuracy_report() -> dict[str, Any]:
    """Live attestation summary for clients (see ACCURACY.md)."""
    attested = [e for e in corpus.entries if e.attested]
    reconstructed = [e for e in corpus.entries if not e.attested]
    flagged = [e.to_dict() for e in corpus.entries if "accuracy_flag" in e.tags]
    by_conf = {"high": 0, "medium": 0, "low": 0}
    for e in corpus.entries:
        by_conf[e.confidence] = by_conf.get(e.confidence, 0) + 1
    doc = (ROOT / "ACCURACY.md").read_text(encoding="utf-8") if (ROOT / "ACCURACY.md").exists() else ""
    return {
        "version": corpus.meta.get("version", "0.3.1"),
        "accuracy_audit": corpus.meta.get("accuracy_audit"),
        "disclaimer": corpus.meta.get("disclaimer")
        or "Prefer colonial anchors; label Neo-Taíno and uncertain Caribbean Spanish clearly.",
        "notes": corpus.meta.get("notes"),
        "counts": {
            "lexicon": len(corpus.entries),
            "attested": len(attested),
            "reconstructed_or_uncertain": len(reconstructed),
            "accuracy_flagged": len(flagged),
            "by_confidence": by_conf,
            "colonial_sentences": len(corpus.sentences.get("sentences", [])),
            "learner_sentences": len(corpus.sentences.get("learner_sentences", [])),
        },
        "flagged_ids": [e["id"] for e in flagged],
        "flagged": flagged[:40],
        "word_order_policy": corpus.grammar.get("meta", {}).get("word_order"),
        "doc_excerpt": "\n".join(doc.splitlines()[:24]),
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


@app.get("/v1/define/{term}")
def define(term: str) -> dict[str, Any]:
    return fun.define(term)


@app.get("/v1/grammar")
def grammar() -> dict[str, Any]:
    return corpus.grammar


@app.get("/v1/sentences")
def sentences_bank() -> dict[str, Any]:
    return corpus.sentences


@app.get("/v1/sentences/structure")
def sentence_structure() -> dict[str, Any]:
    return sentences.overview()


@app.get("/v1/sentences/patterns")
def sentence_patterns(level: int | None = None) -> dict[str, Any]:
    rows = sentences.list_patterns(level=level)
    return {"count": len(rows), "patterns": rows}


@app.get("/v1/sentences/lesson")
def sentence_lesson(pattern_id: str | None = None) -> dict[str, Any]:
    return sentences.lesson(pattern_id)


@app.post("/v1/sentences/build")
def sentence_build(req: SentenceBuildRequest) -> dict[str, Any]:
    return sentences.build(
        pattern_id=req.pattern_id,
        subject=req.subject,
        verb=req.verb,
        obj=req.object,
        noun=req.noun,
        quality=req.quality,
        place=req.place,
        recipient=req.recipient,
        predicate=req.predicate,
        vocative=req.vocative,
    )


@app.get("/v1/sentences/quiz")
def sentence_quiz(n: int = 5) -> dict[str, Any]:
    return {"cards": sentences.quiz(n)}


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


# --- Fun learning modes ---


@app.get("/v1/fun/menu")
def fun_menu() -> dict[str, Any]:
    return fun.play_menu()


@app.get("/v1/fun/word-of-the-day")
def word_of_the_day() -> dict[str, Any]:
    return fun.word_of_the_day()


@app.get("/v1/fun/flashcards")
def flashcards(n: int = 8, tag: str | None = None) -> dict[str, Any]:
    return fun.flashcards(n=n, tag=tag)


@app.get("/v1/fun/match")
def match_game(n: int = 4, tag: str | None = None) -> dict[str, Any]:
    return fun.match_game(n=n, tag=tag)


@app.get("/v1/fun/fill-blank")
def fill_blank(n: int = 5) -> dict[str, Any]:
    return fun.fill_blank(n=n)


@app.get("/v1/fun/story")
def story_quest() -> dict[str, Any]:
    return fun.story_quest()


@app.get("/v1/fun/daily")
def daily_challenge() -> dict[str, Any]:
    return fun.daily_challenge()


@app.get("/v1/fun/sentence-scramble")
def sentence_scramble(n: int = 3) -> dict[str, Any]:
    return fun.sentence_scramble(n=n)


@app.post("/v1/fun/grade")
def grade(req: GradeRequest) -> dict[str, Any]:
    return fun.grade(req.mode, req.answer, req.expected)


@app.post("/v1/fun/progress")
def progress(req: ProgressRequest) -> dict[str, Any]:
    return fun.progress_preview(xp=req.xp, streak=req.streak)


def main() -> None:
    import uvicorn

    uvicorn.run("api.server:app", host="0.0.0.0", port=8080, reload=False)


if __name__ == "__main__":
    main()
