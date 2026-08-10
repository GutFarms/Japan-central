"""Conversational tutor helpers for Boriken learning."""

from __future__ import annotations

import random
from typing import Any

from .corpus import Corpus, load_corpus
from .reconstruct import ReconstructionEngine


LESSON_SKILLS = [
    "greetings",
    "identity",
    "home",
    "nature",
    "food",
    "kinship",
    "grammar_possessive",
    "grammar_negation",
]


class TutorEngine:
    def __init__(self, corpus: Corpus | None = None) -> None:
        self.corpus = corpus or load_corpus()
        self.recon = ReconstructionEngine(self.corpus)

    def lesson(self, skill: str | None = None) -> dict[str, Any]:
        skill = skill or random.choice(LESSON_SKILLS)
        if skill == "greetings":
            examples = [
                ("Taí wey", "Good day", "Buenos días"),
                ("Taíno tí", "Good spirit be with you", "Que el buen espíritu te acompañe"),
                ("Waibá", "Let's go", "Vámonos"),
            ]
            prompt = "Say 'good day' in Boriken."
            answer = "Taí wey"
        elif skill == "identity":
            examples = [
                ("Taíno daka", "I am Taíno / I am good", "Soy taíno"),
                ("Wa-borikén", "Our native land", "Nuestra tierra nativa"),
                ("Borikén", "Puerto Rico / native land", "Puerto Rico"),
            ]
            prompt = "How do you say 'I am Taíno'?"
            answer = "Taíno daka"
        elif skill == "grammar_possessive":
            examples = [
                ("da-bohío", "my house", "mi casa"),
                ("wa-borikén", "our Borikén", "nuestro Borikén"),
                ("li-kanowa", "his canoe", "su canoa"),
                ("to-isa", "her child", "su hijo/a"),
            ]
            prompt = "Build 'my house' using da-."
            answer = "da-bohío"
        elif skill == "grammar_negation":
            examples = [
                ("ma-ni", "without water", "sin agua"),
                ("makabuka", "it is not important", "no importa"),
                ("mayani", "do not kill", "no mates"),
            ]
            prompt = "Make 'without gold' using ma- + kawóna."
            answer = "ma-kawóna"
        elif skill == "food":
            examples = [(e.boriken, e.english, e.spanish) for e in self.corpus.by_tag("food")[:4]]
            prompt = "What is cassava bread in Boriken?"
            answer = "kasabi"
        elif skill == "nature":
            examples = [(e.boriken, e.english, e.spanish) for e in self.corpus.by_tag("nature")[:4]]
            prompt = "Translate 'sun' (attested form)."
            answer = "wey"
        elif skill == "kinship":
            examples = [(e.boriken, e.english, e.spanish) for e in self.corpus.by_tag("kinship")[:4]]
            prompt = "How do you say 'our father'?"
            answer = "wakía baba"
        else:  # home
            examples = [(e.boriken, e.english, e.spanish) for e in self.corpus.by_tag("home")[:4]]
            prompt = "Translate 'house'."
            answer = "bohío"

        return {
            "skill": skill,
            "title": f"Boriken lesson: {skill.replace('_', ' ')}",
            "explanation": self._explain_skill(skill),
            "examples": [
                {"boriken": b, "english": e, "spanish": s} for b, e, s in examples if b
            ],
            "practice": {"prompt": prompt, "answer": answer},
        }

    def _explain_skill(self, skill: str) -> str:
        return {
            "greetings": "Start with warm everyday phrases used in Neo-Taíno community learning.",
            "identity": "Borikén means native land. Taíno originally meant good/noble.",
            "home": "Bohío is the classic dwelling; kaney is the chiefly rectangular house.",
            "nature": "Many nature terms survive in Caribbean Spanish (huracán, sabana, ceiba).",
            "food": "Cassava (yuka) and kasabi were staple foods of Borikén.",
            "kinship": "Possessive prefixes attach directly to kinship stems.",
            "grammar_possessive": "da- my, wa- our, li- his, to-/tu- her.",
            "grammar_negation": "ma- marks absence/negation; ka- marks having/with.",
        }.get(skill, "Practice core Boriken forms with transparent morphology.")

    def chat(self, message: str) -> dict[str, Any]:
        msg = message.strip()
        lower = msg.lower()

        if any(k in lower for k in ("hello", "hi", "hola", "taiguey", "taí wey")):
            return {
                "reply_boriken": "Taí wey! Taíno tí.",
                "reply_english": "Good day! Good spirit be with you.",
                "reply_spanish": "¡Buenos días! Que el buen espíritu te acompañe.",
                "correction": None,
                "suggestion": self.lesson("greetings"),
            }

        if lower.startswith("translate:") or lower.startswith("traduce:"):
            payload = msg.split(":", 1)[1].strip()
            result = self.recon.translate(payload)
            return {
                "reply_boriken": result.boriken,
                "reply_english": result.english or payload,
                "reply_spanish": result.spanish,
                "correction": None,
                "result": result.to_dict(),
            }

        if "?" in msg or lower.startswith("what") or lower.startswith("cómo") or lower.startswith("que"):
            explained = self.recon.explain(msg.replace("?", ""))
            if explained["matches"]:
                m = explained["matches"][0]
                return {
                    "reply_boriken": m["boriken"],
                    "reply_english": m["english"],
                    "reply_spanish": m["spanish"],
                    "correction": None,
                    "result": explained,
                }

        # Treat as attempted Boriken or English to translate
        result = self.recon.translate(msg)
        lesson = self.lesson()
        return {
            "reply_boriken": result.boriken or "Ahiya wakía.",
            "reply_english": (
                f"I read that as: {result.english or msg}. "
                f"Try this practice: {lesson['practice']['prompt']}"
            ),
            "reply_spanish": result.spanish or "Practiquemos boriken juntos.",
            "correction": None,
            "result": result.to_dict(),
            "suggestion": lesson,
        }

    def quiz(self, n: int = 5) -> list[dict[str, Any]]:
        pool = [e for e in self.corpus.entries if e.pos in {"noun", "verb", "phrase", "adjective"}]
        sample = random.sample(pool, k=min(n, len(pool)))
        cards = []
        for e in sample:
            cards.append(
                {
                    "prompt": f"Translate to Boriken: {e.english.split(';')[0].strip()}",
                    "answer": e.boriken,
                    "spanish": e.spanish,
                    "attested": e.attested,
                    "confidence": e.confidence,
                }
            )
        return cards
