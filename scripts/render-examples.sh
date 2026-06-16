#!/usr/bin/env bash
set -euo pipefail

# Render every checked-in example tape into examples/output and mirror the generated files under
# target/betamax-examples. The tracked examples/output paths match the tape files, while
# target/betamax-examples gives README asset upload a stable generated-artifact directory.
#
# Prefer `mise run render-examples` so Cargo uses the repository toolchain, including the Zig
# 0.15.2 version required by libghostty-vt-sys.
cd "$(dirname "$0")/.."

mkdir -p examples/output
mkdir -p target/betamax-examples

cargo run --quiet -- validate examples/*.tape

for tape in examples/*.tape; do
  echo "rendering ${tape}"
  cargo run --quiet -- run --quiet "${tape}"
done

cp -R examples/output/. target/betamax-examples/

find examples/output -maxdepth 1 -type f | sort
find target/betamax-examples -maxdepth 2 \( -type f -o -type d \) | sort
