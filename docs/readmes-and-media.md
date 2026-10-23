# READMEs and media

Betamax has several entry pages, and each one has a different job. Detail, including install
options and the Zig toolchain, belongs on the [docs site](https://www.joshka.net/betamax/). The
READMEs link to it instead of repeating it.

## Entry pages

| File                             | Shown on              | Job                                                     |
| -------------------------------- | --------------------- | ------------------------------------------------------- |
| `README.md`                      | GitHub repository     | Point each reader to the right page; map the repository |
| `crates/betamax/README.md`       | crates.io `betamax`   | Show what Betamax does and how, then how to install it  |
| `crates/betamax-core/README.md`  | crates.io and docs.rs | Show library use for Rust tests and tools               |
| `crates/betamax-fonts/README.md` | crates.io             | Say what the crate is for in one paragraph              |

The GitHub README is a signpost: a one-sentence description, the hero recording, a table of where to
go for each task, and the repository layout. It has no install steps, feature list, or tape
walkthrough; those live on the crates.io page and the docs site.

The crates.io README advertises the CLI: the pitch, the hero recording, the feature bullets, a tape
with what each part does, and a short install section. Install lists `cargo binstall` and Homebrew
and says that `cargo install` works but needs Zig 0.15.2. Everything else links to the docs site.

The feature bullets name what Betamax does that VHS does not. Leave out work that only matches VHS,
such as GIF size; that belongs in [Differences from VHS](vhs-differences.md) if anywhere.

When a feature changes, update its site page and the [Tape reference][tape-reference]. The
READMEs change only when the pitch, the install commands, or the site's structure change.

## crates.io rendering

crates.io renders Markdown differently from GitHub:

- Use absolute URLs for links and images. Relative links do not resolve.
- GitHub alerts such as `> [!NOTE]` render as plain block quotes.
- Raw HTML is mostly stripped. Use Markdown images, not `<picture>` or `<img>` tags.

`README.md` is GitHub-only, so relative links to repository files are fine there.

## Hero recording

Two tapes record the same quick-start flow: installing Betamax, creating a tape, and rendering it.
Keep their commands in step.

- `examples/quick-start.tape` is the README hero. It uses the current default look and writes
  `quick-start.gif`, which both READMEs load from the `readme-assets` release (see
  [Example media](#example-media)).
- `site/quick-start.tape` is the homepage version. The homepage shows it inside a TV frame image, so
  the tape is frameless. It writes `site/public/assets/betamax-hero-quick-start.gif`.

Both tapes run `cargo binstall betamax`, so they need `cargo-binstall` and network access, and they
install the released binary into `~/.cargo/bin`. The homepage tape puts `target/debug` first on
`PATH`, so its later commands show the local build:

```sh
mise exec -- cargo run -- run site/quick-start.tape
```

The homepage GIF is stored with Git LFS (see `.gitattributes`). jj does not run LFS filters, so from
a jj checkout, commit the LFS pointer instead of the image bytes:

```sh
git lfs clean -- site/public/assets/betamax-hero-quick-start.gif \
  < site/public/assets/betamax-hero-quick-start.gif > pointer
mv pointer site/public/assets/betamax-hero-quick-start.gif
git lfs push origin --object-id <oid from the pointer>
```

The docs workflow deploys the site when the change reaches `main`.

## Example media

Tapes under `examples/` are smoke tests and documentation examples. Render them with:

```sh
mise run render-examples
```

Output goes to `examples/output` and is copied to `target/betamax-examples`. Generated media is not
tracked in the repository.

The `readme-assets` GitHub Release hosts the rendered GIFs. Both READMEs load `quick-start.gif`
from it. The crates.io README and the homepage also load `btop.gif`, rendered from
`examples/btop.tape`, which needs `btop` installed. The site's
[Examples](https://www.joshka.net/betamax/examples/) page loads several more.
Earlier published crate READMEs also link there, so keep the release and its asset names.

Refresh the assets with:

```sh
mise run upload-readme-assets
```

That command renders the examples first. It needs the [GitHub CLI](https://cli.github.com/) and
permission to upload release assets.

[tape-reference]: https://www.joshka.net/betamax/reference/tape-reference/
