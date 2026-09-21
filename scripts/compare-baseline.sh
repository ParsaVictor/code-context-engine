#!/usr/bin/env bash
# compare-baseline.sh <binary> <NEUROMESH_HOME> <set> <repo-name> <checkout-dir> <gold_tasks.toml>
# Scores one gold set through a binary's own `optimize` output, by file name, so two engine builds compare on equal terms (docs/baseline-vs-fork-2026-09-21.txt).
# gold_files/forbidden_files. Prints: set/repo tasks recall precision forbidden
set -uo pipefail
bin="$1"; home="$2"; set="$3"; name="$4"; dir="$5"; gold="$6"
cd "$dir" || exit 1
NEUROMESH_HOME="$home" "$bin" index >/dev/null 2>&1
n=0; rsum=0; psum=0; forb=0
while IFS= read -r line; do
  case "$line" in
    "prompt = "*) prompt="${line#prompt = }"; prompt="${prompt#\"}"; prompt="${prompt%\"}";;
    "gold_files = "*) g="${line#gold_files = }"; goldb=$(echo "$g" | grep -o '"[^"]*"' | tr -d '"' | xargs -n1 basename 2>/dev/null | sort -u);;
    "forbidden_files = "*) f="${line#forbidden_files = }"; forbb=$(echo "$f" | grep -o '"[^"]*"' | tr -d '"' | xargs -n1 basename 2>/dev/null | sort -u)
      out=$(NEUROMESH_HOME="$home" "$bin" optimize "$prompt" 2>/dev/null | awk '/^Files \(/{f=1;next} f&&/^  /{print $1} f&&!/^  /{f=0}' | sort -u)
      [ -z "$out" ] && out=$(NEUROMESH_HOME="$home" "$bin" optimize --query "$prompt" 2>/dev/null | awk '/^Files \(/{f=1;next} f&&/^  /{print $1} f&&!/^  /{f=0}' | sort -u)
      gc=$(echo "$goldb" | grep -c .); oc=$(echo "$out" | grep -c .)
      hit=$(comm -12 <(echo "$goldb") <(echo "$out") | grep -c .)
      fh=$(comm -12 <(echo "$forbb") <(echo "$out") | grep -c .)
      r=$(awk -v h=$hit -v g=$gc 'BEGIN{printf "%.3f", g?h/g:0}')
      p=$(awk -v h=$hit -v o=$oc 'BEGIN{printf "%.3f", o?h/o:0}')
      n=$((n+1)); rsum=$(awk -v a=$rsum -v b=$r 'BEGIN{print a+b}'); psum=$(awk -v a=$psum -v b=$p 'BEGIN{print a+b}'); forb=$((forb+fh))
      ;;
  esac
done < "$gold"
awk -v s="$set/$name" -v n=$n -v r=$rsum -v p=$psum -v f=$forb 'BEGIN{printf "%-28s tasks=%2d recall=%.3f precision=%.3f forbidden=%d\n", s, n, n?r/n:0, n?p/n:0, f}'
