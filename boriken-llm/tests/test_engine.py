"""Unit tests for Boriken reconstruction + fun learning."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from engine.corpus import load_corpus
from engine.fun import FunLearnEngine
from engine.reconstruct import ReconstructionEngine
from engine.sentence import SentenceStructureEngine
from engine.tutor import TutorEngine


class CorpusTests(unittest.TestCase):
    def setUp(self) -> None:
        load_corpus.cache_clear()
        self.corpus = load_corpus()

    def test_lexicon_not_empty(self) -> None:
        self.assertGreater(len(self.corpus.entries), 50)

    def test_lookup_boriken(self) -> None:
        hits = self.corpus.lookup("borikén")
        self.assertTrue(hits)
        self.assertIn("native", hits[0].english.lower())

    def test_definitions_present(self) -> None:
        missing = [e.id for e in self.corpus.entries if not e.definition_en]
        self.assertEqual(missing, [], f"Missing definitions: {missing[:5]}")
        self.assertTrue(self.corpus.lookup("hurricane")[0].fun_fact)

    def test_lookup_english_loanword(self) -> None:
        hits = self.corpus.lookup("hurricane")
        self.assertTrue(hits)
        self.assertTrue(hits[0].attested)


class ReconstructionTests(unittest.TestCase):
    def setUp(self) -> None:
        load_corpus.cache_clear()
        self.engine = ReconstructionEngine()

    def test_translate_attested(self) -> None:
        r = self.engine.translate("canoe")
        self.assertEqual(r.boriken, "kanowa")
        self.assertEqual(r.confidence, "high")
        self.assertTrue(r.definition_en)

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


class TutorTests(unittest.TestCase):
    def setUp(self) -> None:
        load_corpus.cache_clear()
        self.tutor = TutorEngine()

    def test_lesson(self) -> None:
        lesson = self.tutor.lesson("identity")
        self.assertEqual(lesson["skill"], "identity")
        self.assertTrue(lesson["examples"])
        self.assertIn("fun_hook", lesson)

    def test_quiz(self) -> None:
        cards = self.tutor.quiz(3)
        self.assertEqual(len(cards), 3)

    def test_chat_greeting(self) -> None:
        reply = self.tutor.chat("hola")
        self.assertIn("Taí", reply["reply_boriken"])


class FunTests(unittest.TestCase):
    def setUp(self) -> None:
        load_corpus.cache_clear()
        self.fun = FunLearnEngine()

    def test_word_of_day(self) -> None:
        w = self.fun.word_of_the_day("2026-08-10")
        self.assertIn("word", w)
        self.assertTrue(w["word"]["definition_en"])

    def test_match_game(self) -> None:
        g = self.fun.match_game(n=3)
        self.assertEqual(len(g["left"]), 3)
        self.assertEqual(len(g["answer_key"]), 3)

    def test_grade(self) -> None:
        ok = self.fun.grade("match", "kanowa", "kanowa")
        self.assertTrue(ok["correct"])
        bad = self.fun.grade("match", "nope", "kanowa")
        self.assertFalse(bad["correct"])

    def test_story_quest(self) -> None:
        s = self.fun.story_quest()
        self.assertGreaterEqual(len(s["beats"]), 3)

    def test_progress(self) -> None:
        p = self.fun.progress_preview(xp=60, streak=3)
        self.assertGreaterEqual(p["level"], 2)
        self.assertTrue(any(b["unlocked"] for b in p["badges"]))

    def test_sentence_scramble(self) -> None:
        g = self.fun.sentence_scramble(n=2)
        self.assertEqual(g["mode"], "sentence_scramble")
        self.assertTrue(g["cards"])


class SentenceStructureTests(unittest.TestCase):
    def setUp(self) -> None:
        load_corpus.cache_clear()
        self.ss = SentenceStructureEngine()

    def test_overview(self) -> None:
        ov = self.ss.overview()
        self.assertIn("SVO", ov["word_order"])
        self.assertGreaterEqual(ov["pattern_count"], 10)

    def test_build_svo(self) -> None:
        r = self.ss.build("svo_present", subject="I", verb="drink", obj="water")
        self.assertTrue(r["ok"])
        self.assertIn("da-", r["boriken"])
        self.assertIn("SUBJECT-VERB", r["structure"])

    def test_build_identity(self) -> None:
        r = self.ss.build("identity", predicate="Taíno")
        self.assertEqual(r["boriken"].split()[-1], "daka")

    def test_build_possession(self) -> None:
        r = self.ss.build("possessed_noun", subject="our", noun="house")
        self.assertTrue(r["boriken"].startswith("wa-"))

    def test_lesson(self) -> None:
        lesson = self.ss.lesson("svo_present")
        self.assertEqual(lesson["pattern"]["id"], "svo_present")
        self.assertIn("practice", lesson)

    def test_tutor_sentence_skill(self) -> None:
        tutor = TutorEngine()
        lesson = tutor.lesson("sentence_structure")
        self.assertEqual(lesson["skill"], "sentence_structure")
        self.assertIn("pattern", lesson)


if __name__ == "__main__":
    unittest.main()
