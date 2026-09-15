"""Build instruction-tuning and language-model corpora for BorikenLLM."""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from engine.corpus import load_corpus, iter_training_pairs  # noqa: E402


def build_instruct_jsonl(out_path: Path) -> int:
    corpus = load_corpus()
    rows = []
    system = (ROOT / "prompts" / "system.md").read_text(encoding="utf-8")

    for pair in iter_training_pairs(corpus):
        rows.append(
            {
                "messages": [
                    {"role": "system", "content": system},
                    {"role": "user", "content": pair["instruction"]},
                    {"role": "assistant", "content": pair["output"]},
                ],
                "meta": pair["meta"],
            }
        )

    # Morphology drills
    drills = [
        ("Form 'my canoe' in Boriken.", "da-kanowa"),
        ("Form 'our house' in Boriken.", "wa-bohío"),
        ("Form 'his gold' in Boriken.", "li-kawóna"),
        ("Form 'her child' in Boriken.", "to-isa"),
        ("Form 'without rain' in Boriken.", "ma-yukubiya"),
        ("Form 'having spirit' in Boriken.", "ka-semí"),
        ("Translate: Let's go.", "Waibá"),
        ("Translate: I am God's worker.", "Dios naboriya daka"),
        ("Translate: Speak to us, our grandfather.", "Ahiyakawo, warokoel"),
        ("What does ma- mean?", "negative / without / un-"),
        ("What does ka- mean?", "having / with / characterized by"),
        ("What does da- mean?", "my / I (1sg)"),
        ("What does wa- mean?", "our / we (1pl)"),
        ("Explain Borikén.", "bori 'native' + -ken/-ke 'land' → native land (Puerto Rico)"),
    ]
    for user, assistant in drills:
        rows.append(
            {
                "messages": [
                    {"role": "system", "content": system},
                    {"role": "user", "content": user},
                    {"role": "assistant", "content": assistant},
                ],
                "meta": "drill",
            }
        )

    # Tutor dialogues
    dialogues = [
        (
            "Teach me a greeting.",
            "Taí wey means Good day. Practice: say Taí wey to someone this morning.",
        ),
        (
            "How do I say I love Borikén?",
            "Anichi borikén. (reconstructed composition; anichi≈love, borikén=native land)",
        ),
        (
            "Is huracán Taíno?",
            "Yes — hurakán is attested and entered Spanish/English as hurricane.",
        ),
    ]
    for user, assistant in dialogues:
        rows.append(
            {
                "messages": [
                    {"role": "system", "content": system},
                    {"role": "user", "content": user},
                    {"role": "assistant", "content": assistant},
                ],
                "meta": "tutor",
            }
        )

    out_path.parent.mkdir(parents=True, exist_ok=True)
    with out_path.open("w", encoding="utf-8") as f:
        for row in rows:
            f.write(json.dumps(row, ensure_ascii=False) + "\n")
    return len(rows)


def build_lm_text(out_path: Path) -> int:
    corpus = load_corpus()
    lines = corpus.all_parallel_lines()
    # Add grammar statements for LM pretraining
    lines.extend(
        [
            "da- means my or I",
            "wa- means our or we",
            "li- means his or he",
            "to- means her or she",
            "ma- means without or not",
            "ka- means having or with",
            "Borikén is the native land",
            "Taíno daka means I am good",
            "Waibá means lets go",
            "O kãma, waxeri, warikẽ kawóna yari",
            "Ahiyakawo, warokoel",
            "Dios naboriya daka",
        ]
    )
    text = "\n".join(lines) + "\n"
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(text, encoding="utf-8")
    return len(lines)


def main() -> None:
    n1 = build_instruct_jsonl(ROOT / "corpus" / "training" / "instruct.jsonl")
    n2 = build_lm_text(ROOT / "corpus" / "training" / "lm_corpus.txt")
    print(f"Wrote {n1} instruct rows and {n2} LM lines")


if __name__ == "__main__":
    main()
