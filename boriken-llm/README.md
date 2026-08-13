# BorikenLLM

A language reconstruction **LLM + API + iOS client** for rebuilding **Borikén Taíno** (the Indigenous Arawakan language of Puerto Rico) inside an iOS app.

## What you get

| Piece | Role |
|---|---|
| `corpus/` | Curated lexicon, grammar, attested sentences, training data |
| `engine/` | Deterministic reconstruction (morphology, translation, composition) |
| `train/` | Dataset builder + tiny **BorikenGPT** character language model |
| `api/` | FastAPI server the iOS app calls |
| `ios/BorikenKit` | Swift package client |
| `ios/SampleApp` | SwiftUI starter screen |

This is a **community language-rebuilding toolkit**. Attested colonial forms are preferred and labeled `attested`. Neo-Taíno / comparative reconstructions are labeled clearly so the app never pretends certainty.

# Download BorikenLLM

See **[DOWNLOAD.md](DOWNLOAD.md)** for packages:

| Package | File |
|---|---|
| **High-graphics desktop** | `dist/Boriken-Desktop-linux.tar.gz` |
| Offline learner (iPhone) | `dist/Boriken-Offline-Learner.zip` |
| iOS Xcode content bundle | `dist/Boriken-iOS-ContentBundle.zip` |
| Full API + model toolkit | `dist/BorikenLLM-Toolkit.zip` |

```bash
# Desktop from source
cd desktop && cargo run --release

# Or rebuild all downloadables
./scripts/package.sh
```

## Quick start

```bash
cd boriken-llm
python3 -m pip install -r requirements.txt
python3 train/build_dataset.py
python3 train/train_lm.py --steps 800
python3 -m uvicorn api.server:app --host 0.0.0.0 --port 8080
```

Health check:

```bash
curl -s http://127.0.0.1:8080/health | python3 -m json.tool
```

Translate:

```bash
curl -s -X POST http://127.0.0.1:8080/v1/translate \
  -H 'content-type: application/json' \
  -d '{"text":"my house"}' | python3 -m json.tool
```

## Fun learning (iOS)

Modes under `/v1/fun/*`:

| Mode | Endpoint | Vibe |
|---|---|---|
| Word of the Day | `GET /v1/fun/word-of-the-day` | One island word + XP quest |
| Memory Bohío | `GET /v1/fun/flashcards` | Flip cards with definitions + fun facts |
| Batey Match | `GET /v1/fun/match` | Match Boriken ↔ meaning |
| Konuko Fill-In | `GET /v1/fun/fill-blank` | Plant the missing word |
| Areyto Quest | `GET /v1/fun/story` | Tiny choose-your-path adventure |
| Daily Island Run | `GET /v1/fun/daily` | Three-step streak combo |

Definitions: every lexicon entry now has `definition_en`, `definition_es`, `fun_fact`, and `example`. Use `GET /v1/define/{term}`.

The sample SwiftUI home (`ios/SampleApp/BorikenHomeView.swift`) plays Word of the Day, Batey Match, lessons, XP, and streaks.

## How the “LLM” works

1. In Xcode, **File → Add Package Dependencies…** and add the local package `boriken-llm/ios/BorikenKit`.
2. Point `BorikenClient` at your API host (simulator: `http://127.0.0.1:8080`, device: your LAN/Tailscale/production URL).
3. Drop in `ios/SampleApp/BorikenHomeView.swift` or call the client directly:

```swift
let client = BorikenClient(baseURL: URL(string: "https://api.example.com")!)
let result = try await client.translate("hurricane")
print(result.boriken) // hurakán
```

### Useful endpoints

- `GET /health`
- `GET /v1/lexicon?q=canoe`
- `GET /v1/grammar`
- `POST /v1/translate` `{ "text": "our land" }`
- `POST /v1/reconstruct` `{ "concept": "smartphone" }`
- `POST /v1/chat` `{ "message": "hola" }`
- `POST /v1/lesson` `{ "skill": "identity" }`
- `GET /v1/quiz?n=5`
- `POST /v1/complete` — local BorikenGPT completion
- `GET /v1/system-prompt` — prompt for cloud LLM backends

## How the “LLM” works

1. **BorikenGPT** — small character-level transformer trained on the Boriken parallel corpus (`models/boriken-gpt.pt`). Good for completions and on-device research scaffolding.
2. **Reconstruction engine** — reliable path for the app: lexicon lookup, possessive/subject composition (`da-`, `wa-`, `li-`, `to-`), negation (`ma-`), attributive (`ka-`), and phonotactic loan adaptation.
3. **System prompt** (`prompts/system.md`) — plug into OpenAI / Apple Foundation Models / any chat backend for richer tutoring while grounding answers in `/v1/lexicon` + `/v1/explain`.

For production iOS, keep the deterministic engine as source of truth for flashcards/translation, and use the neural/chat layer for explanations and practice dialogue.

## Grammar anchors taught by the model
- `da-` my/I · `wa-` our/we · `li-` his · `to-` her
- `ma-` without/not · `ka-` having/with
- Default learner **sentence structure = SVO** (`da-sá ni` = I drink water)
- `taíno daka` — I am good / I am Taíno
- `waibá` — let’s go
- `wa-borikén` — our native land

API: `GET /v1/sentences/structure`, `GET /v1/sentences/patterns`, `POST /v1/sentences/build`

## Expanding the lexicon

Edit `corpus/vocabulary.json` (set `"attested": true/false` and optional `"source"`), then rebuild:

```bash
python3 train/build_dataset.py
python3 train/train_lm.py --steps 800
```

## Tests

```bash
python3 -m unittest discover -s tests -v
```

## Respect

Borikén language work is cultural reclamation. This repo ships transparent confidence labels so learners and descendants can see what is colonial-attested versus modern reconstruction. Prefer community dictionaries (e.g. Hiwatahia–Hekexi, Borikenaíki primers, Living Dictionaries) when integrating larger lexica—respect their licenses and authorship.
