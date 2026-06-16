#!/usr/bin/env bash
set -euo pipefail

# Package the already-built Betamax CLI for cargo-binstall.
#
# cargo-binstall package metadata:
# https://github.com/cargo-bins/cargo-binstall/blob/main/SUPPORT.md
#
# Usage:
#
#   scripts/package-binstall-archive.sh <target-triple> <version> [dist-dir]
#
# Run `mise run build-release -- <target-triple>` first. The script intentionally does not build
# anything itself, so the GitHub workflow controls the Rust target, locked dependencies, and mise
# toolchain before packaging starts.
target="${1:?target triple is required}"
version="${2:?version is required}"
dist_dir="${3:-target/dist}"

binary="target/${target}/release/betamax"
if [[ ! -x "$binary" ]]; then
  echo "missing release binary: $binary" >&2
  exit 1
fi

work_dir="${dist_dir}/betamax-${version}-${target}"
archive="${dist_dir}/betamax-${version}-${target}.tgz"

rm -rf "$work_dir" "$archive"
mkdir -p "$work_dir" "$dist_dir"

cp "$binary" "$work_dir/betamax"
tar -C "$work_dir" -czf "$archive" betamax
echo "$archive"
