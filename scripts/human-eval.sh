#!/usr/bin/env bash
# Human evaluation without an API key (session 10, decision 5b).
#
# Builds one packet per question from a TOML task file and writes them as
# Markdown for a person to rate. The repository, the questions and the
# packets stay wherever you point them — nothing is written into this tree,
# so it works for a private repository.
#
#   bash scripts/human-eval.sh <repo> <tasks.toml> <out-dir>
#
# Rate each packet with one of: enough / missing / too-much, in the sheet the
# script writes (<out-dir>/ratings.md). The result is the cheapest proxy for
# task success we have while phase C waits for a key.
set -euo pipefail
repo="$1"; tasks="$2"; out="$3"
root="$(cd "$(dirname "$0")/.." && pwd)"
bin="$root/target/release/neuromesh"
[ -x "$bin" ] || { echo "build first: CARGO_BUILD_JOBS=2 cargo build --release -p neuromesh-cli" >&2; exit 1; }
mkdir -p "$out"
(cd "$repo" && "$bin" index >/dev/null 2>&1)
{
  echo "# Human eval — $(basename "$repo") — $(date +%F)"
  echo
  echo "| # | id | files | tokens | rating (enough / missing / too-much) | note |"
  echo "|---|---|---|---|---|---|"
} > "$out/ratings.md"
n=0
id=""; prompt=""
flush() {
  [ -n "$id" ] || return 0
  n=$((n+1))
  (cd "$repo" && "$bin" packet "$prompt" --json) > "$out/$id.json" 2>/dev/null || true
  files=$(node -e 'const p=JSON.parse(require("fs").readFileSync(process.argv[1],"utf8"));console.log(p.selected_files_count??"?")' "$(cygpath -w "$out/$id.json" 2>/dev/null || echo "$out/$id.json")" 2>/dev/null || echo "?")
  tokens=$(node -e 'const p=JSON.parse(require("fs").readFileSync(process.argv[1],"utf8"));console.log(p.packet_tokens??"?")' "$(cygpath -w "$out/$id.json" 2>/dev/null || echo "$out/$id.json")" 2>/dev/null || echo "?")
  {
    echo "# $id"; echo; echo "**Q:** $prompt"; echo
    (cd "$repo" && "$bin" optimize "$prompt" --full 2>/dev/null) || true
  } > "$out/$id.md"
  echo "| $n | $id | $files | $tokens |  |  |" >> "$out/ratings.md"
  id=""; prompt=""
}
while IFS= read -r line; do
  line="${line%$'\r'}"
  case "$line" in
    "[[task]]") flush ;;
    id\ =*) id=$(echo "$line" | sed 's/^id *= *"\(.*\)"/\1/') ;;
    prompt\ =*) prompt=$(echo "$line" | sed 's/^prompt *= *"\(.*\)"/\1/') ;;
  esac
done < "$tasks"
flush
echo "wrote $n packets to $out; rate them in $out/ratings.md"
