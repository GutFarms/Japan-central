# Download BorikenLLM

Four ready-to-download packages (also under `dist/` after `./scripts/package.sh`):

| Package | Best for | File |
|---|---|---|
| **Desktop (high graphics)** | Cinematic offline classroom on Linux | `Boriken-Desktop-linux.tar.gz` |
| **Offline Learner** | Instant play on iPhone/desktop — no server | `Boriken-Offline-Learner.zip` |
| **iOS Content Bundle** | Drop into Xcode (Swift kit + corpus + model) | `Boriken-iOS-ContentBundle.zip` |
| **Full Toolkit** | Run the API + retrain + ship the app | `BorikenLLM-Toolkit.zip` |

## Desktop (highest graphics)

```bash
tar -xzf Boriken-Desktop-linux.tar.gz
cd Boriken-Desktop-*-linux
./boriken-desktop
```

Ubuntu/Debian once if the window fails:

```bash
sudo apt install libxkbcommon-x11-0 libxcb-xkb1
```

Includes animated **Sol Taíno**, cemí figures, coquí & carey petroglyphs,
ocean parallax, particle glyph weather, XP games, and the full defined lexicon.

Build from source:

```bash
cd boriken-llm/desktop
cargo run --release
```

## Quickest path (iPhone)

1. Download **Boriken-Offline-Learner.zip**
2. Unzip → open `index.html` in Safari  
3. Share → **Add to Home Screen**
4. Play Word of the Day, Batey Match, Areyto Quest offline

## iOS app developers

1. Download **Boriken-iOS-ContentBundle.zip**
2. Add local Swift package `BorikenKit/`
3. Copy `Corpus/*.json` into the app target
4. Use `BorikenHomeView.swift` or `BorikenClient`

Optional API (from Toolkit zip):

```bash
python3 -m pip install -r requirements.txt
python3 -m uvicorn api.server:app --host 0.0.0.0 --port 8080
```

## Build packages yourself

```bash
cd boriken-llm
./scripts/package.sh
```

Artifacts from agent runs are also copied to `/opt/cursor/artifacts/`.
