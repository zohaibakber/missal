#!/usr/bin/env bash
set -euo pipefail

binary="${1:?usage: smoke.sh <missal binary> [seconds]}"
seconds="${2:-8}"
scratch="$(mktemp -d)"
pid=""
trap '[ -n "$pid" ] && kill "$pid" 2>/dev/null; pkill -f -- "--data-dir $scratch/data" 2>/dev/null; rm -rf "$scratch"' EXIT

mkdir -p "$scratch/bin" "$scratch/data"
cp "$binary" "$scratch/bin/missal"
cd "$scratch/bin"

./missal --data-dir "$scratch/data" --seed 3
test -s "$scratch/data/missal.db"

xvfb-run -a -s "-screen 0 1600x1000x24" ./missal --data-dir "$scratch/data" &
pid=$!
for _ in $(seq 1 $((seconds * 10))); do
    if ! kill -0 "$pid" 2>/dev/null; then
        wait "$pid" || true
        echo "missal exited before ${seconds}s" >&2
        exit 1
    fi
    sleep 0.1
done
echo "ok: ran ${seconds}s from an empty directory against a scratch data directory"
