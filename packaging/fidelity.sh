#!/usr/bin/env bash
set -euo pipefail

if [ $# -lt 2 ]; then
  echo "usage: packaging/fidelity.sh OUT_DIR FILE.docx..." >&2
  exit 2
fi

out=$1
shift
root=$(cd "$(dirname "$0")/.." && pwd)
render="$root/target/release/examples/render_pages"
mkdir -p "$out"
if [ ! -x "$render" ] || [ -n "${REBUILD:-}" ]; then
  cargo build --release -q -p missal-page --features headless --example render_pages
fi

for docx in "$@"; do
  name=$(basename "$docx" .docx)
  work="$out/$name"
  rm -rf "$work"
  mkdir -p "$work/word" "$work/missal"
  soffice --headless --convert-to pdf --outdir "$work" "$docx" >/dev/null 2>&1
  pdftoppm -r 96 -png "$work/$name.pdf" "$work/word/page"
  "$render" --docx "$docx" --all --zoom 1 --fir --out "$work/missal" >/dev/null
  index=0
  for reference in "$work"/word/page-*.png; do
    ours=$(printf "%s/missal/page-%02d.png" "$work" "$index")
    pair=$(printf "%s/pair-%02d.png" "$work" "$index")
    if [ -f "$ours" ]; then
      size=$(identify -format "%wx%h!" "$reference")
      convert "$reference" \( "$ours" -resize "$size" \) +append "$pair"
    else
      convert "$reference" -bordercolor red -border 6 "$pair"
    fi
    index=$((index + 1))
  done
  word_pages=$(find "$work/word" -name 'page-*.png' | wc -l)
  missal_pages=$(find "$work/missal" -name 'page-*.png' | wc -l)
  echo "$name: Word reference $word_pages pages, Missal $missal_pages pages"
done
