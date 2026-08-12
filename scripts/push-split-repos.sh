#!/usr/bin/env bash
# Push rebuilt orphan histories to GutFarms/<name> remotes.
set -euo pipefail

ORG="${ORG:-GutFarms}"
REMOTE_BASE="${REMOTE_BASE:-https://github.com/${ORG}}"

# japan-central stays this repo; push emblem rebuild to master via normal PR merge.
# remote_name:local_split_ref_suffix
REPOS=(
  Native:native
  farm-manager:farm-manager
  boriken-llm:boriken-llm
  cyd-miner:cyd-miner
  esp32-scrypt-miner:esp32-scrypt-miner
  jchc1-hardware:jchc1-hardware
  cyd-pc-monitor:cyd-pc-monitor
  cursor-appliance:cursor-appliance
  grok-agent:grok-agent
  pi-invest-agent:pi-invest-agent
  pi-invest-os:pi-invest-os
)

fail=0
for entry in "${REPOS[@]}"; do
  name="${entry%%:*}"
  ref_suffix="${entry##*:}"
  ref="split/${ref_suffix}"
  if ! git rev-parse -q --verify "$ref" >/dev/null; then
    echo "MISSING local ref $ref" >&2
    fail=1
    continue
  fi
  url="${REMOTE_BASE}/${name}.git"
  # New GitHub repos default to main (often with a stub README); force-replace with orphan tip.
  target_branch="${TARGET_BRANCH:-main}"
  echo "→ $url  ($ref → ${target_branch})"
  if git push -u "$url" "+${ref}:refs/heads/${target_branch}"; then
    echo "OK $name"
  else
    echo "FAIL $name (create empty repo + grant Cursor App write access, then retry)" >&2
    fail=1
  fi
done

# Also refresh Japan-central master from split/japan-central when explicitly requested
if [[ "${PUSH_JAPAN_CENTRAL_MASTER:-0}" == "1" ]]; then
  echo "→ origin master from split/japan-central"
  git push origin "+split/japan-central:refs/heads/master" || fail=1
fi

exit "$fail"
