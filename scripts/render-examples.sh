#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

cargo_cmd=(cargo)
if command -v mise >/dev/null 2>&1; then
  cargo_cmd=(mise exec -- cargo)
fi

mkdir -p examples/output
mkdir -p target/betamax-examples

"${cargo_cmd[@]}" run --quiet -- validate examples/*.tape

for tape in examples/*.tape; do
  echo "rendering ${tape}"
  "${cargo_cmd[@]}" run --quiet -- run --quiet "${tape}"
done

cp -R examples/output/. target/betamax-examples/

find examples/output -maxdepth 1 -type f | sort
find target/betamax-examples -maxdepth 2 \( -type f -o -type d \) | sort
