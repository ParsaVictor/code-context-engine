#!/usr/bin/env bash
# Same numbers as benchmark-holdout.sh, faster: the gold harnesses are built
# once with optimisations and the sets run side by side (NM_JOBS at a time,
# default 3; each set indexes its own checkouts, so memory is the limit).
#
#   bash scripts/benchmark-fast.sh                      # the nine default sets
#   bash scripts/benchmark-fast.sh concept concept-holdout holdout-lang
#
# Output: one summary line per set, in the order given; per-set logs in
# target/bench-fast/<set>.log.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
export NM_THIRD_PARTY=1
jobs="${NM_JOBS:-3}"

declare -A MANIFEST=(
  [dev]="tests/third_party/repos.toml"
  [large]="tests/third_party/large/repos.toml"
  [holdout]="tests/third_party/holdout/repos.toml"
  [holdout-c]="tests/third_party/holdout-c/repos.toml"
  [holdout-lang]="tests/third_party/holdout-lang/repos.toml"
  [holdout-ml]="tests/third_party/holdout-ml/repos.toml"
  [holdout-ml2]="tests/third_party/holdout-ml2/repos.toml"
  [holdout-cfg]="tests/third_party/holdout-cfg/repos.toml"
  [holdout-web]="tests/third_party/holdout-web/repos.toml"
  [concept-holdout]="tests/third_party/concept-holdout/repos.toml"
  [concept-holdout2]="tests/third_party/concept-holdout2/repos.toml"
  [concept-holdout3]="tests/third_party/concept-holdout3/repos.toml"
  [concept-holdout4]="tests/third_party/concept-holdout4/repos.toml"
  [concept-holdout5]="tests/third_party/concept-holdout5/repos.toml"
)
declare -A TEST=(
  [dev]="third_party_gold"
  [large]="third_party_large_gold"
  [holdout]="third_party_holdout_gold"
  [holdout-c]="third_party_c_holdout_gold"
  [holdout-lang]="third_party_lang_holdout_gold"
  [holdout-ml]="third_party_ml_holdout_gold"
  [holdout-ml2]="third_party_ml2_holdout_gold"
  [holdout-cfg]="third_party_cfg_holdout_gold"
  [holdout-web]="third_party_web_holdout_gold"
  [concept]="third_party_private_gold"
  [concept-holdout]="third_party_private_gold"
  [concept-holdout2]="third_party_private_gold"
  [concept-holdout3]="third_party_private_gold"
  [concept-holdout4]="third_party_private_gold"
  [concept-holdout5]="third_party_private_gold"
)
sets=("$@")
if [ ${#sets[@]} -eq 0 ]; then
  sets=(dev large holdout holdout-c holdout-lang holdout-ml holdout-ml2 holdout-cfg holdout-web)
fi

for set in "${sets[@]}"; do
  case "$set" in
    concept) ;;
    dev) bash scripts/fetch-third-party.sh >/dev/null ;;
    *) bash scripts/fetch-third-party.sh "${MANIFEST[$set]}" "$set" >/dev/null ;;
  esac
done

# Build every needed harness once, optimised; then run the binaries directly.
tests=()
for set in "${sets[@]}"; do tests+=(--test "${TEST[$set]}"); done
cargo test --release -q -p neuromesh-context "${tests[@]}" --no-run 2>/dev/null
bin_of() {
  # newest optimised binary for a harness name
  # absolute: run_set changes directory before executing it
  ls -t "$root"/target/release/deps/"$1"-*.exe "$root"/target/release/deps/"$1"-* 2>/dev/null \
    | grep -Ev '\.(d|pdb|exp|lib|rlib)$' | head -1
}

out="target/bench-fast"
mkdir -p "$out"
run_set() {
  local set="$1" bin
  bin="$(bin_of "${TEST[$set]}")"
  local envs=()
  case "$set" in
    concept) envs=(NM_PRIVATE_SET_DIR="$root/tests/third_party/concept" NM_PRIVATE_DIR="$root/..") ;;
    concept-holdout) envs=(NM_PRIVATE_SET_DIR="$root/tests/third_party/concept-holdout" NM_PRIVATE_DIR="$root/target/third_party/concept-holdout") ;;
    concept-holdout2) envs=(NM_PRIVATE_SET_DIR="$root/tests/third_party/concept-holdout2" NM_PRIVATE_DIR="$root/target/third_party/concept-holdout2") ;;
    concept-holdout3) envs=(NM_PRIVATE_SET_DIR="$root/tests/third_party/concept-holdout3" NM_PRIVATE_DIR="$root/target/third_party/concept-holdout3") ;;
    concept-holdout4) envs=(NM_PRIVATE_SET_DIR="$root/tests/third_party/concept-holdout4" NM_PRIVATE_DIR="$root/target/third_party/concept-holdout4") ;;
    concept-holdout5) envs=(NM_PRIVATE_SET_DIR="$root/tests/third_party/concept-holdout5" NM_PRIVATE_DIR="$root/target/third_party/concept-holdout5") ;;
  esac
  # The harness resolves the workspace from its manifest dir at build time.
  (cd crates/neuromesh-context && env "${envs[@]}" "$bin" --nocapture >"$root/$out/$set.log" 2>&1 || true)
}

running=0
for set in "${sets[@]}"; do
  run_set "$set" &
  running=$((running + 1))
  if [ "$running" -ge "$jobs" ]; then
    wait -n
    running=$((running - 1))
  fi
done
wait

echo "set                 recall  precision  forbidden  oracle(reachable/strict)"
for set in "${sets[@]}"; do
  line=$(grep -E "^third_party" "$out/$set.log" | tail -1 || true)
  recall=$(echo "$line" | sed -n 's/.*mean recall \([0-9.]*\).*/\1/p')
  prec=$(echo "$line" | sed -n 's/.*precision \([0-9.]*\).*/\1/p')
  forb=$(echo "$line" | sed -n 's/.*forbidden hits \([0-9]*\).*/\1/p')
  reach=$(echo "$line" | sed -n 's/.*reachable \([0-9]*\).*/\1/p')
  strict=$(echo "$line" | sed -n 's/.*strict \([0-9]*\).*/\1/p')
  cases=$(echo "$line" | sed -n 's/.*: \([0-9]*\) gold cases.*/\1/p')
  printf "%-19s %-7s %-10s %-10s %s/%s strict %s\n" "$set" "${recall:-?}" "${prec:-?}" "${forb:-?}" "${reach:-?}" "${cases:-?}" "${strict:-?}"
done
