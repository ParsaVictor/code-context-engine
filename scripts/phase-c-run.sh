#!/usr/bin/env bash
# Phase C, end to end, resumable. Needs ANTHROPIC_API_KEY in the shell (and
# ANTHROPIC_BASE_URL / ANTHROPIC_USER_AGENT for a gateway). Runs the fixture
# tasks and the holdout tasks under packet / whole-gold-files / grep, writing
# reports/phase-c/<set>-<ctx>.json. A file whose run hit a provider error is
# rerun; a clean one is kept, so the script can be restarted at any time.
#
#   MODEL=claude-opus-5 JUDGE=claude-opus-4-8 bash scripts/phase-c-run.sh
set -uo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
bin="$root/target/release/neuromesh"
# Answerer/judge pairs, tried in order per context: a pair whose run hit a
# provider error is abandoned and the next pair reruns that context from the
# start, so one report never mixes models. PAIRS="a:b c:d" overrides.
pairs="${PAIRS:-deepseek-v4-flash:glm-5.3 glm-5.3:deepseek-v4-flash claude-opus-5:claude-opus-4-8 gpt-6-astra:gpt-5.6-sol}"
out="$root/reports/phase-c"; mkdir -p "$out"
export NM_TASK_REPLY_DIR="${NM_TASK_REPLY_DIR:-$root/target/phase-c-replies}"
bash "$root/scripts/fetch-third-party.sh" tests/third_party/holdout/repos.toml holdout >/dev/null
clean() { [ -s "$1" ] && grep -q "^Task success" "$1" && ! grep -q "provider error" "$1"; }
run() { # set workdir tasksfile
  local set="$1" dir="$2" tasks="$3"
  for ctx in packet whole-gold-files grep; do
    local kept=""
    for pair in $pairs; do
      local model="${pair%%:*}"
      if clean "$out/$set-$ctx-$model.json"; then kept="$model"; break; fi
    done
    if [ -n "$kept" ]; then echo "keep  $set/$ctx $kept"; continue; fi
    for pair in $pairs; do
      local model="${pair%%:*}" judge="${pair##*:}"
      local f="$out/$set-$ctx-$model.json"
      echo "run   $set/$ctx $model (judge $judge)"
      (cd "$dir" && "$bin" eval --tasks --executor model --context "$ctx"           --model "$model" --judge-model "$judge" ${tasks:+--tasks-file "$tasks"} --json) > "$f" 2>"$out/$set-$ctx-$model.log"
      if clean "$f"; then grep -E "^Task success" "$f" | cut -c1-160; break; fi
      echo "fail  $set/$ctx $model - trying next pair"
    done
  done
}
run fixtures "$root" ""
run holdout-gin "$root/target/third_party/holdout" "$root/tests/third_party/holdout/gin/tasks.toml"
run holdout-vision "$root/target/third_party/holdout" "$root/tests/third_party/holdout/vision/tasks.toml"
echo "done"
