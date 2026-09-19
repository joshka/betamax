# PR-built CLI previews

Linux's **Package and smoke** CI job builds the checked-out Betamax CLI with
`cargo build -p betamax --locked`, then passes `target/debug/betamax` to
[`joshka/betamax-action`](https://github.com/joshka/betamax-action)'s `binary` input.
This bypasses the action's release installer. The build log records the executable's version and
SHA-256 for diagnosis; it is not an attestation from a trusted runner.

The every-PR selection combines the [feature field guide](feature-gallery.md) with one small
format/timing smoke tape. Seven named GIF/PNG scenes demonstrate input, keys, styled Unicode,
light/dark framing, captions, overlays, capture visibility and scrollback/alternate screens. The
field guide records their assertions, coverage map, known gaps and reproduction instructions.

`examples/ci-playback.tape` independently checks Hide/Show, frame changes and unequal one/two-second
holds at normal speed in GIF, PNG, MP4 and WebM. `scripts/check-action-playback.py` requires exactly
that successful tape result and all four formats, then checks decoded dimensions (480×240), codecs,
video pixel format and duration. Decoded samples bracket the red-to-blue transition and the final
blue hold. The final PNG must be blue. GIF/MP4/WebM duration must be 2.9–3.4 seconds, allowing frame
and encoder rounding around three seconds. Sleeps provide visible holds after semantic waits.

Open **Terminal previews → Open gallery** in the job summary for the action's media gallery, or
**Betamax field guide** for descriptions, expected results and checkpoint comparisons. Individual
media, state JSON and action diagnostics are also run artifacts. GitHub sign-in is required, and
preview artifacts expire after 14 days. Linux/macOS tests, macOS CLI smoke testing and the specialized
[renderer fidelity PNG/JSON galleries](renderer-fidelity.md) remain in place.

## Playback-speed regression coverage

[Issue 149](https://github.com/joshka/betamax/issues/149) identified unscaled elapsed capture
holds: `PlaybackSpeed 2` produced roughly three seconds of GIF, MP4, WebM, and WebP output for
this fixture instead of approximately 1.5 seconds. PTY capture now divides elapsed holds by
playback speed while keeping command execution and capture cadence unchanged.

The focused runner regression decodes GIF and WebP at speeds 0.5, 1, and 2, checking the color
transition and total duration across both `Sleep` and visible `Wait` capture. Run it with:

```sh
mise exec -- cargo test -p betamax-core --test playback_speed
```

`mise run video-test` also checks decoded MP4/WebM transitions and the final hold at those speeds.
The action fixture stays at normal speed; these focused tests provide the non-default coverage.
To inspect speed 2 manually with the locally built CLI:

```sh
sed 's/Set PlaybackSpeed 1/Set PlaybackSpeed 2/' examples/ci-playback.tape \
  | target/debug/betamax run --quiet --output target/playback-speed-2.mp4 -
ffprobe -v error -show_entries format=duration -of default=nw=1 target/playback-speed-2.mp4
```

Expect approximately 1.55 seconds, including one frame for each `Show` command.

## Trust boundary

The PR build and render job runs on a disposable GitHub-hosted runner with `contents: read`,
`persist-credentials: false` and no secrets. PR code, build scripts, tapes and the resulting binary
can compromise that entire runner. The action's path checks, output checks and playback assertions
are diagnostics, not a sandbox or a security boundary. Never add a PAT or publication credential to
any later step of that job.

The **Renderer gallery comment** workflow runs after CI using trusted default-branch configuration.
Its native preview job runs on a fresh runner with no checkout, PR binary or restored cache. The
reporter uses the same reviewed commit pin as the rendering action. It checks source workflow, run,
current PR head and artifact metadata. `GITHUB_TOKEN` has `actions: read` and `pull-requests: write`
and owns the comment, which includes gallery links and inline media.

Only this native preview job receives `BETAMAX_ATTACHMENT_TOKEN`, from the `betamax-attachments`
environment. That environment permits the branch `main` explicitly, with no tag or PR-ref rules.
The token must be restricted to Betamax and must not also exist as a repository secret: otherwise
a same-repository PR could edit its workflow to request the repository-level copy. A token scoped
to `joshka/betamax-action` must not be reused here. Required reviewers can be added to the environment
if publishing each run should need approval; branch restriction alone keeps reporting automatic.

Native mode downloads untrusted media and sends the PAT only to GitHub's upload endpoint. The
pinned reporter independently checks allowed download hosts, artifact digests, byte limits and
media signatures; it never extracts archives or executes downloaded files. These checks constrain
the upload path but do not prove media harmless to every downstream decoder. Neither artifact
digests nor passing checks on the compromised render runner establish that its content is safe.
The custom PNG/JSON gallery reporter remains in its own job without the PAT or artifact downloads.

## Configure the upload credential

In repository settings, create `betamax-attachments` with **Selected branches and tags**, allowing
only branch `main`. Save the Betamax-scoped PAT there as `BETAMAX_ATTACHMENT_TOKEN`, then remove any
repository-level copy. GitHub cannot return an existing secret's value, so enter the original PAT
directly in the environment settings or through `gh secret set --env betamax-attachments`; do not
put it in source, logs or a PR comment. Verify the environment rule and secret before activation.

## Verify a workflow change

Run the workflow and Markdown checks locally:

```sh
actionlint .github/workflows/ci.yml .github/workflows/renderer-comment.yml
zizmor .github/workflows/ci.yml .github/workflows/renderer-comment.yml
mise exec -- markdownlint-cli2 docs/action-previews.md
```

On the PR, inspect the Linux build log for the local binary path and hash, the action log for local
executable selection, and the decoded-frame/timing check. Open the resulting gallery and play both
animations. A new `workflow_run` reporter step only activates once its workflow is on the default
branch. After merging, use a subsequent PR run to verify the **Terminal previews** comment and its
inline media. Check the reporter log for upload warnings: individual failures fall back to artifact
links and can leave the job green. Rerun the source CI workflow to get a new run attempt and test
comment updates; rerunning only the reporter skips an already reported attempt. Before activation, the
PR run's gallery and media artifacts provide live rendering evidence, but do not prove the new
reporter step ran in this repository.
