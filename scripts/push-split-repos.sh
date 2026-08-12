#!/usr/bin/env bash
# Push rebuilt orphan histories to GutFarms/<name> remotes.
set -euo pipefail

ORG="${ORG:-GutFarms}"
REMOTE_BASE="${REMOTE_BASE:-https://github.com/${ORG}}"

# japan-central stays this repo; push emblem rebuild to master via normal PR merge.
REPOS=(
  native
  farm-manager
  boriken-llm
  cyd-miner
  esp32-scrypt-miner
  jchc1-hardware
  cyd-pc-monitor
  cursor-appliance
  grok-agent
  pi-invest-agent
  pi-invest-os
)

fail=0
for name in "${REPOS[@]}"; do
  ref="split/${name}"
  if ! git rev-parse -q --verify "$ref" >/dev/null; then
    echo "MISSING local ref $ref" >&2
    fail=1
    continue
  fi
  url="${REMOTE_BASE}/${name}.git"
  echo "→ $url  ($ref → master)"
  if git push -u "$url" "+${ref}:refs/heads/master"; then
    echo "OK $name"
  else
    echo "FAIL $name (create empty repo + grant app access, then retry)" >&2
    fail=1
  fi
done

# Also refresh Japan-central master from split/japan-central when explicitly requested
if [[ "${PUSH_JAPAN_CENTRAL_MASTER:-0}" == "1" ]]; then
  echo "→ origin master from split/japan-central"
  git push origin "+split/japan-central:refs/heads/master" || fail=1
fi

exit "$fail"
