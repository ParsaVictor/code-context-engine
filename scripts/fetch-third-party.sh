#!/usr/bin/env bash
# Fetch the pinned third-party checkouts listed in a repos.toml manifest
# into $NM_THIRD_PARTY_DIR (default target/third_party). Idempotent: a
# checkout already at the pinned revision is left alone.
#
# Usage: fetch-third-party.sh [repos.toml] [dest-subdir]
#   fetch-third-party.sh                                            # dev repos (default)
#   fetch-third-party.sh tests/third_party/holdout/repos.toml holdout  # phase 5a holdout repos
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
manifest="${1:-$root/tests/third_party/repos.toml}"
case "$manifest" in
  /*) : ;;
  *) manifest="$root/$manifest" ;;
esac
dest="${NM_THIRD_PARTY_DIR:-$root/target/third_party}"
if [ -n "${2:-}" ]; then
  dest="$dest/$2"
fi
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
  # With core.autocrlf=true the toml is checked out with CRLF; a trailing
  # \r made "[[repo]]" never match, so only the last repo was ever fetched.
  line="${line%$'\r'}"
  case "$line" in
    "[[repo]]") flush ;;
    name*) name="$(echo "$line" | sed 's/^name *= *"\(.*\)"/\1/')" ;;
    url*)  url="$(echo "$line" | sed 's/^url *= *"\(.*\)"/\1/')" ;;
    rev*)  rev="$(echo "$line" | sed 's/^rev *= *"\(.*\)"/\1/')" ;;
  esac
done < "$manifest"
flush
