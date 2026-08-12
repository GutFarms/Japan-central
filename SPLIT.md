# Split plan — Japan-central → separate repos

## Why

Cursor agents landed many unrelated products as PR branches on **Japan-central**. This rebuild keeps **Japan-central** as the Japan & Central clock and extracts each product into its own repository with a **fresh orphan git history** (content preserved; tangled shared history discarded).

## Mapping (PR → new repo)

| Old PRs | Source tip | New repo |
|---------|------------|----------|
| #15 (closed) + master | `cursor/emblem-thought-spin-724a` | **Japan-central** (this repo) |
| #14, #16, #17, #18 | `cursor/split-to-native-repo-2cb7` | `GutFarms/native` |
| #3, #19 | `cursor/farm-kmz-api-import-e801` | `GutFarms/farm-manager` |
| #13 | `cursor/boriken-language-llm-404a` | `GutFarms/boriken-llm` |
| #12 (+ older companion branch) | `cursor/esp32-cyd-cpp-firmware-e801` | `GutFarms/cyd-miner` |
| #1, #2, #4 | `cursor/esp32-2432s028-cyd-e801` (+ AGENTS.md from #4) | `GutFarms/esp32-scrypt-miner` |
| #5 (closed), #6 | `cursor/esp32-asic-controller-board-e801` | `GutFarms/jchc1-hardware` |
| #9 | `cursor/esp32-cyd-pc-monitor-9f0c` | `GutFarms/cyd-pc-monitor` |
| #11 | `cursor/local-cursor-appliance-d337` | `GutFarms/cursor-appliance` |
| #10 | `cursor/grok-local-agent-50bd` | `GutFarms/grok-agent` |
| #7 | `cursor/pi-invest-agent-0b6b` | `GutFarms/pi-invest-agent` |
| #8 | `cursor/pi-invest-os-0b6b` | `GutFarms/pi-invest-os` |

## Local refs / bundles

- Branches: `split/<repo-name>` on this remote
- Bundles: `/opt/cursor/artifacts/split-bundles/<repo-name>.bundle`

## Push to new GitHub repos

1. Create each empty repo under **GutFarms** (no README / license / gitignore).
2. Grant the Cursor GitHub App access.
3. Run:

```bash
./scripts/push-split-repos.sh
```

Or manually:

```bash
git push https://github.com/GutFarms/native.git split/native:master
# …repeat for each repo
```

## After push

Close obsolete Japan-central PRs (#1–#14, #16–#19) with a comment pointing at the new repo. Keep release assets where they already are until re-tagged on the new remotes.
