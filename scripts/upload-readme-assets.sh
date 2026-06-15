#!/usr/bin/env bash
set -euo pipefail

# Refresh the generated README preview media on the `readme-assets` GitHub Release. Keeping these
# files as release assets avoids committing generated media while still giving README and docs pages
# stable URLs.
#
# Prefer `mise run upload-readme-assets`. Pass an alternate release tag as the first argument only
# when testing the upload flow against a throwaway release.
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
  target/betamax-examples/quick-start.gif \
  target/betamax-examples/basic.gif \
  target/betamax-examples/hide-show.gif \
  target/betamax-examples/themes.gif \
  target/betamax-examples/video.gif \
  --clobber
