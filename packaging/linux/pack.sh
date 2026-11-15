#!/usr/bin/env bash
set -euo pipefail

version="${1:?usage: pack.sh <version> [output directory]}"
output="${2:-dist-native}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

stage="target/stage-linux"
rm -rf "$stage"
mkdir -p "$stage"
cp target/release/missal "$stage/"
cp packaging/THIRD-PARTY-NOTICES.txt assets/fonts/Inter-LICENSE.txt "$stage/"

vpk [linux] pack \
    --packId Missal \
    --packVersion "$version" \
    --packDir "$stage" \
    --mainExe missal \
    --packTitle Missal \
    --packAuthors "Zohaib Akber" \
    --icon assets/icon.png \
    --categories Office \
    --channel linux \
    --outputDir "$output"
