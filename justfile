set positional-arguments

default:
    @just --list

fmt:
    mise exec -- cargo +nightly fmt --all

fmt-check:
    mise exec -- cargo +nightly fmt --all --check

test:
    mise exec -- cargo test --workspace --all-targets

clippy:
    mise exec -- cargo clippy --workspace --all-targets -- -D warnings

doc-test:
    mise exec -- cargo test --workspace --doc

doc:
    mise exec -- cargo doc --workspace --no-deps --document-private-items

lint-md:
    mise exec -- markdownlint-cli2 '**/*.md' '#target'

outdated:
    mise exec -- cargo outdated --workspace --root-deps-only

minimal-versions:
    mise exec -- cargo minimal-versions --direct --workspace check

audit:
    mise exec -- cargo audit

feature-check:
    mise exec -- cargo hack check --workspace --all-targets

udeps:
    mise exec -- cargo +nightly udeps --workspace --all-targets

validate:
    mise exec -- cargo run -p betamax -- validate 'examples/*.tape' docs.tape

smoke:
    mise exec -- cargo run -- run examples/basic.tape

render-examples:
    mise exec -- scripts/render-examples.sh

upload-readme-assets:
    mise exec -- scripts/upload-readme-assets.sh

package-core:
    mise exec -- cargo package -p betamax-core --allow-dirty

package-cli:
    # betamax depends on betamax-core from crates.io after packaging. Before the core 0.1.0
    # release exists on crates.io, only workspace packaging can assemble the CLI tarball.
    mise exec -- cargo package --workspace --allow-dirty --no-verify

package-cli-verify:
    mise exec -- cargo package -p betamax --allow-dirty

package: package-core package-cli

publish-dry-run-core:
    mise exec -- cargo publish -p betamax-core --dry-run --allow-dirty

publish-dry-run-cli:
    # This succeeds only after betamax-core 0.1.0 is available from crates.io.
    mise exec -- cargo publish -p betamax --dry-run --allow-dirty

install-smoke:
    #!/usr/bin/env bash
    set -euo pipefail
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    CARGO_HOME="$tmp/cargo-home" mise exec -- cargo install --path crates/betamax --locked --root "$tmp/install"
    "$tmp/install/bin/betamax" --version

check: fmt-check clippy test doc-test doc lint-md validate

release-check: check audit feature-check outdated minimal-versions package install-smoke smoke
