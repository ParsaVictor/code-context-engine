#!/usr/bin/env bash
# One command for the numbers the project reports: every third-party gold
# set, dev and holdout, in one run. Prints one summary line per set.
#
#   bash scripts/benchmark-holdout.sh            # all sets
#   bash scripts/benchmark-holdout.sh holdout    # one set: dev|large|holdout|holdout-c|holdout-lang|holdout-ml|private
#
# "private" is the phase-5b holdout on a repository that is not in this tree:
# set NM_PRIVATE_SET_DIR (manifest + gold) and NM_PRIVATE_DIR (checkouts); it is
# skipped silently when they are unset and is never part of the default run.
#
# Sets that need a checkout are fetched first (pinned revisions, idempotent).
# Windows note: this machine needs CARGO_BUILD_JOBS=2 to avoid an rustc ICE.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export NM_THIRD_PARTY=1

declare -A MANIFEST=(
  [dev]="tests/third_party/repos.toml"
  [large]="tests/third_party/large/repos.toml"
  [holdout]="tests/third_party/holdout/repos.toml"
  [holdout-c]="tests/third_party/holdout-c/repos.toml"
  [holdout-lang]="tests/third_party/holdout-lang/repos.toml"
  [holdout-ml]="tests/third_party/holdout-ml/repos.toml"
)
declare -A TEST=(
  [dev]="third_party_gold"
  [large]="third_party_large_gold"
  [holdout]="third_party_holdout_gold"
  [holdout-c]="third_party_c_holdout_gold"
  [holdout-lang]="third_party_lang_holdout_gold"
  [holdout-ml]="third_party_ml_holdout_gold"
  [private]="third_party_private_gold"
)
sets=("$@")
if [ ${#sets[@]} -eq 0 ]; then sets=(dev large holdout holdout-c holdout-lang holdout-ml); fi
for set in "${sets[@]}"; do
  if [ "$set" = private ]; then
    [ -n "${NM_PRIVATE_SET_DIR:-}" ] || { echo "private: NM_PRIVATE_SET_DIR unset; skipping" >&2; }
    continue
  fi
  if [ "$set" = dev ]; then
    bash scripts/fetch-third-party.sh >/dev/null
  else
    bash scripts/fetch-third-party.sh "${MANIFEST[$set]}" "$set" >/dev/null
  fi
done

echo "set                 recall  precision  forbidden  oracle(reachable/strict)"
for set in "${sets[@]}"; do
  line=$(cargo test -q -p neuromesh-context --test "${TEST[$set]}" -- --nocapture 2>&1 \
    | grep -E "^third_party" | tail -1 || true)
  # third_party_x: N gold cases mean recall R precision P forbidden hits F; N task cases reachable A strict S
  recall=$(echo "$line" | sed -n 's/.*mean recall \([0-9.]*\).*/\1/p')
  prec=$(echo "$line" | sed -n 's/.*precision \([0-9.]*\).*/\1/p')
  forb=$(echo "$line" | sed -n 's/.*forbidden hits \([0-9]*\).*/\1/p')
  reach=$(echo "$line" | sed -n 's/.*reachable \([0-9]*\).*/\1/p')
  strict=$(echo "$line" | sed -n 's/.*strict \([0-9]*\).*/\1/p')
  cases=$(echo "$line" | sed -n 's/.*: \([0-9]*\) gold cases.*/\1/p')
  printf "%-19s %-7s %-10s %-10s %s/%s strict %s\n" "$set" "${recall:-?}" "${prec:-?}" "${forb:-?}" "${reach:-?}" "${cases:-?}" "${strict:-?}"
done
