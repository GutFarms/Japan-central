# Push split histories (run on your machine)

The cloud agent’s `cursor[bot]` token can push to `Japan-central` only. Until the Cursor GitHub App is granted write access to the new repos, push from your own GitHub login:

```bash
git clone https://github.com/GutFarms/Japan-central.git
cd Japan-central
git fetch origin \
  split/native split/farm-manager split/boriken-llm split/cyd-miner \
  split/esp32-scrypt-miner split/jchc1-hardware split/cyd-pc-monitor \
  split/cursor-appliance split/grok-agent split/pi-invest-agent split/pi-invest-os

git push -f https://github.com/GutFarms/Native.git             origin/split/native:main
git push -f https://github.com/GutFarms/farm-manager.git       origin/split/farm-manager:main
git push -f https://github.com/GutFarms/boriken-llm.git        origin/split/boriken-llm:main
git push -f https://github.com/GutFarms/cyd-miner.git          origin/split/cyd-miner:main
git push -f https://github.com/GutFarms/esp32-scrypt-miner.git origin/split/esp32-scrypt-miner:main
git push -f https://github.com/GutFarms/jchc1-hardware.git     origin/split/jchc1-hardware:main
git push -f https://github.com/GutFarms/cyd-pc-monitor.git     origin/split/cyd-pc-monitor:main
git push -f https://github.com/GutFarms/cursor-appliance.git   origin/split/cursor-appliance:main
git push -f https://github.com/GutFarms/grok-agent.git         origin/split/grok-agent:main
git push -f https://github.com/GutFarms/pi-invest-agent.git    origin/split/pi-invest-agent:main
git push -f https://github.com/GutFarms/pi-invest-os.git       origin/split/pi-invest-os:main
```

Or fix App access: https://github.com/organizations/GutFarms/settings/installations → Cursor → **All repositories** → Save.
