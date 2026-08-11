#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"
JAR="${ROOT}/dist/NativePure-Companion.jar"
if [[ ! -f "$JAR" ]]; then
  echo "Missing $JAR — build with: ./gradlew packageUberJarForCurrentOS" >&2
  echo "Then: mkdir -p dist && cp build/compose/jars/NativePureCompanion-*-*.jar dist/NativePure-Companion.jar" >&2
  exit 1
fi
exec java -jar "$JAR" "$@"
