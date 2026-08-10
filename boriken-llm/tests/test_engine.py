"""Unit tests for Boriken reconstruction engine."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from engine.corpus import load_corpus
from engine.reconstruct import ReconstructionEngine
from engine.tutor import TutorEngine


class CorpusTests(unittest.TestCase):
    def setUp(self) -> None:
        self.corpus = load_corpus()

    def test_lexicon_not_empty(self) -> None:
        self.assertGreater(len(self.corpus.entries), 50)

    def test_lookup_boriken(self) -> None:
        hits = self.corpus.lookup("borikén")
        self.assertTrue(hits)
        self.assertIn("native", hits[0].english.lower())

    def test_lookup_english_loanword(self) -> None:
        hits = self.corpus.lookup("hurricane")
        self.assertTrue(hits)
        self.assertTrue(hits[0].attested)


class ReconstructionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.engine = ReconstructionEngine()

    def test_translate_attested(self) -> None:
        r = self.engine.translate("canoe")
        self.assertEqual(r.boriken, "kanowa")
        self.assertEqual(r.confidence, "high")

    def test_possessive_composition(self) -> None:
        r = self.engine.translate("my house")
        self.assertTrue(r.boriken.startswith("da-"))
        self.assertEqual(r.attestation, "composition")

    def test_negation_composition(self) -> None:
        r = self.engine.translate("without water")
        self.assertTrue(r.boriken.startswith("ma-"))

    def test_explain_prefix(self) -> None:
        info = self.engine.explain("da-bohío")
        self.assertTrue(any("1sg" in m or "da-" in m for m in info["morphology"]))

    def test_loan_adaptation(self) -> None:
        adapted = self.engine.adapt_loanword("computer")
        self.assertTrue(adapted)
        self.assertNotIn("c", adapted.replace("ch", ""))  # c→k generally


class TutorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tutor = TutorEngine()

    def test_lesson(self) -> None:
        lesson = self.tutor.lesson("identity")
        self.assertEqual(lesson["skill"], "identity")
        self.assertTrue(lesson["examples"])

    def test_quiz(self) -> None:
        cards = self.tutor.quiz(3)
        self.assertEqual(len(cards), 3)

    def test_chat_greeting(self) -> None:
        reply = self.tutor.chat("hola")
        self.assertIn("Taí", reply["reply_boriken"])


if __name__ == "__main__":
    unittest.main()
