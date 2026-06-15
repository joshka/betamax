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

case "$target" in
  *-apple-darwin)
    lib_pattern="libghostty-vt.0.1.0.dylib"
    library_path_var="DYLD_LIBRARY_PATH"
    ;;
  *-unknown-linux-gnu)
    lib_pattern="libghostty-vt.so.0.1.0"
    library_path_var="LD_LIBRARY_PATH"
    ;;
  *)
    echo "unsupported target for Betamax binary archive: $target" >&2
    exit 1
    ;;
esac

lib="$(find "target/${target}/release/build" -path "*/ghostty-install/lib/${lib_pattern}" -print | head -1)"
if [[ -z "$lib" ]]; then
  echo "missing libghostty-vt shared library for $target" >&2
  exit 1
fi
lib_dir="$(dirname "$lib")"

# cargo-binstall extracts the file named by package.metadata.binstall.bin-dir as the installed
# executable. Betamax would prefer to ship only that executable, but libghostty-vt-sys 0.1.1 links
# the vendored Ghostty VT library dynamically and does not expose a complete static link mode. Build
# a self-extracting launcher named `betamax` instead: cargo-binstall installs the launcher, and the
# launcher unpacks the real binary plus shared library into the user's cache on first run.
work_dir="${dist_dir}/betamax-${version}-${target}"
payload_dir="${work_dir}/payload"
payload_archive="${work_dir}/payload.tgz"
archive="${dist_dir}/betamax-${version}-${target}.tgz"
launcher="${work_dir}/betamax"

rm -rf "$work_dir" "$archive"
mkdir -p "$payload_dir" "$dist_dir"

cp "$binary" "$payload_dir/betamax-real"
find "$lib_dir" -maxdepth 1 \
  \( -name "libghostty-vt*.dylib" -o -name "libghostty-vt.so*" \) \
  -exec cp -P {} "$payload_dir/" \;

cat >"$launcher" <<LAUNCHER
#!/bin/sh
set -eu

version="$version"
target="$target"
library_path_var="$library_path_var"
cache="\${XDG_CACHE_HOME:-\${HOME}/.cache}/betamax/\${version}/\${target}"
real="\${cache}/betamax-real"

if [ ! -x "\$real" ]; then
  tmp="\${cache}.\$\$"
  rm -rf "\$tmp"
  mkdir -p "\$tmp"
  payload_line=\$(awk '/^__BETAMAX_PAYLOAD_BELOW__\$/ { print NR + 1; exit }' "\$0")
  tail -n "+\$payload_line" "\$0" | tar -xz -C "\$tmp"
  chmod 0755 "\$tmp/betamax-real"
  mkdir -p "\$(dirname "\$cache")"
  rm -rf "\$cache"
  mv "\$tmp" "\$cache"
fi

# Keep the dynamic library lookup local to the cached payload, then preserve any caller-provided
# value so users can still layer their own runtime library paths if needed.
case "\$library_path_var" in
  DYLD_LIBRARY_PATH)
    export DYLD_LIBRARY_PATH="\$cache\${DYLD_LIBRARY_PATH:+:\$DYLD_LIBRARY_PATH}"
    ;;
  LD_LIBRARY_PATH)
    export LD_LIBRARY_PATH="\$cache\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}"
    ;;
esac

exec "\$real" "\$@"

__BETAMAX_PAYLOAD_BELOW__
LAUNCHER

# Preserve libghostty-vt's symlink chain in the embedded payload so platform dynamic linkers can
# resolve either the soname or the unversioned library name.
tar -C "$payload_dir" -czf "$payload_archive" .
cat "$payload_archive" >>"$launcher"
chmod 0755 "$launcher"

tar -C "$work_dir" -czf "$archive" betamax
echo "$archive"
