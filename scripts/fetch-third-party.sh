#!/usr/bin/env bash
# Fetch the pinned third-party checkouts listed in tests/third_party/repos.toml
# into $NM_THIRD_PARTY_DIR (default target/third_party). Idempotent: a
# checkout already at the pinned revision is left alone.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
dest="${NM_THIRD_PARTY_DIR:-$root/target/third_party}"
mkdir -p "$dest"
name=""; url=""; rev=""
flush() {
  if [ -n "$name" ]; then
    dir="$dest/$name"
    if [ -d "$dir/.git" ] && [ "$(git -C "$dir" rev-parse HEAD 2>/dev/null)" = "$rev" ]; then
      echo "ok      $name @ ${rev:0:12}"
    else
      rm -rf "$dir"; mkdir -p "$dir"
      git -C "$dir" init -q
      git -C "$dir" remote add origin "$url"
      git -C "$dir" fetch -q --depth 1 origin "$rev"
      git -C "$dir" checkout -q FETCH_HEAD
      echo "fetched $name @ ${rev:0:12}"
    fi
  fi
  name=""; url=""; rev=""
}
while IFS= read -r line; do
  case "$line" in
    "[[repo]]") flush ;;
    name*) name="$(echo "$line" | sed 's/^name *= *"\(.*\)"/\1/')" ;;
    url*)  url="$(echo "$line" | sed 's/^url *= *"\(.*\)"/\1/')" ;;
    rev*)  rev="$(echo "$line" | sed 's/^rev *= *"\(.*\)"/\1/')" ;;
  esac
done < "$root/tests/third_party/repos.toml"
flush
