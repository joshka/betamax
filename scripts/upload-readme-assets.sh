#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

tag="${1:-readme-assets}"

scripts/render-examples.sh

if ! gh release view "$tag" >/dev/null 2>&1; then
  gh release create "$tag" \
    --title "README assets" \
    --notes "Generated Betamax example media for README previews." \
    --latest=false
fi

gh release upload "$tag" \
  target/betamax-examples/basic.gif \
  target/betamax-examples/hide-show.gif \
  target/betamax-examples/themes.gif \
  target/betamax-examples/video.gif \
  --clobber
