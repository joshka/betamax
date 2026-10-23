#!/usr/bin/env bash
set -euo pipefail

# Refresh the generated README preview media on the `readme-assets` GitHub Release. Keeping these
# files as release assets avoids committing generated media while still giving README and docs pages
# stable URLs.
#
# Renders only the tapes whose GIFs are uploaded, so example tapes that need other programs, such as
# the ratatui-diff viewer, don't block a refresh. `quick-start.tape` runs `cargo binstall betamax`,
# and `btop.tape` needs `btop` installed.
#
# Prefer `mise run upload-readme-assets`. Pass an alternate release tag as the first argument only
# when testing the upload flow against a throwaway release.
cd "$(dirname "$0")/.."

tag="${1:-readme-assets}"
assets=(quick-start basic btop hide-show themes video)

mkdir -p examples/output target/betamax-examples

files=()
for name in "${assets[@]}"; do
  echo "rendering examples/${name}.tape"
  cargo run --quiet -- run --quiet "examples/${name}.tape"
  cp "examples/output/${name}.gif" target/betamax-examples/
  files+=("target/betamax-examples/${name}.gif")
done

if ! gh release view "$tag" >/dev/null 2>&1; then
  gh release create "$tag" \
    --title "README assets" \
    --notes "Generated Betamax example media for README previews." \
    --latest=false
fi

gh release upload "$tag" "${files[@]}" --clobber
