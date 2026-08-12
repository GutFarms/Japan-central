# Split plan — Japan-central → separate repos

## Why

Cursor agents landed many unrelated products as PR branches on **Japan-central**. This rebuild keeps **Japan-central** as the Japan & Central clock and extracts each product into its own repository with a **fresh orphan git history** (content preserved; tangled shared history discarded).

## Mapping (PR → new repo)

| Old PRs | Source tip | New repo |
|---------|------------|----------|
| #15 (closed) + master | `cursor/emblem-thought-spin-724a` | **Japan-central** (this repo) |
| #14, #16, #17, #18 | `cursor/split-to-native-repo-2cb7` | **Left on Japan-central** (`split/native` available; `GutFarms/Native` unused) |
| #3, #19 | `cursor/farm-kmz-api-import-e801` | `GutFarms/farm-manager` ✅ |
| #13 | `cursor/boriken-language-llm-404a` | `GutFarms/boriken-llm` ✅ |
| #12 (+ older companion branch) | `cursor/esp32-cyd-cpp-firmware-e801` | `GutFarms/cyd-miner` ✅ |
| #1, #2, #4 | `cursor/esp32-2432s028-cyd-e801` (+ AGENTS.md from #4) | `GutFarms/esp32-scrypt-miner` ✅ |
| #5 (closed), #6 | `cursor/esp32-asic-controller-board-e801` | `GutFarms/jchc1-hardware` ✅ |
| #9 | `cursor/esp32-cyd-pc-monitor-9f0c` | `GutFarms/cyd-pc-monitor` ✅ |
| #11 | `cursor/local-cursor-appliance-d337` | `GutFarms/cursor-appliance` ✅ |
| #10 | `cursor/grok-local-agent-50bd` | `GutFarms/grok-agent` ✅ |
| #7 | `cursor/pi-invest-agent-0b6b` | `GutFarms/pi-invest-agent` ✅ |
| #8 | `cursor/pi-invest-os-0b6b` | `GutFarms/pi-invest-os` ✅ |

## Local refs / bundles

- Branches: `split/<repo-name>` on this remote (including unused `split/native`)
- Bundles: `/opt/cursor/artifacts/split-bundles/<repo-name>.bundle`

## Push helpers

Cloud agents can write only to Japan-central. Product repos were pushed from a local machine (`scripts/push-split-repos-as-user.md`). `scripts/push-split-repos.sh` no longer targets Native.
