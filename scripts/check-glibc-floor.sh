#!/usr/bin/env bash
set -euo pipefail

# Fail if a Linux binary references a glibc symbol version newer than the supported floor.
#
# Usage:
#
#   scripts/check-glibc-floor.sh <binary> <floor>
#
# The release workflow links Linux binaries against the floor with cargo-zigbuild. This check keeps
# a toolchain or workflow change from silently raising the glibc a release needs. Weak references
# count too, which keeps the rule simple: no `GLIBC_x.y` above the floor at all.
binary="${1:?binary is required}"
floor="${2:?glibc floor is required}"

versions="$(objdump -T "$binary" | { grep -oE 'GLIBC_[0-9]+(\.[0-9]+)+' || true; } | sed 's/^GLIBC_//' | sort -uV)"
if [[ -z "$versions" ]]; then
  echo "no glibc symbol versions found in $binary" >&2
  exit 1
fi

newest="$(tail -n1 <<<"$versions")"
if [[ "$(printf '%s\n%s\n' "$floor" "$newest" | sort -V | tail -n1)" != "$floor" ]]; then
  echo "$binary needs glibc $newest, above the $floor floor. Symbols above the floor:" >&2
  while read -r version; do
    if [[ "$(printf '%s\n%s\n' "$floor" "$version" | sort -V | tail -n1)" != "$floor" ]]; then
      objdump -T "$binary" | grep -F "GLIBC_${version})" >&2 || true
    fi
  done <<<"$versions"
  exit 1
fi

echo "$binary needs glibc $newest (floor $floor)"
