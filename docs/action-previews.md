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
and encoder rounding around three seconds. Sleeps hold each frame after its ready marker appears.

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

Previews stay in the run's job summary and artifacts; they are not posted to the PR. The separate
**Renderer gallery comment** workflow only links the [renderer fidelity](renderer-fidelity.md)
galleries, using trusted default-branch configuration and no PR checkout.

## Verify a workflow change

Run the workflow and Markdown checks locally:

```sh
actionlint .github/workflows/ci.yml .github/workflows/renderer-comment.yml
zizmor .github/workflows/ci.yml .github/workflows/renderer-comment.yml
mise exec -- markdownlint-cli2 docs/action-previews.md
```

On the PR, inspect the Linux build log for the local binary path and hash, the action log for local
executable selection, and the decoded-frame/timing check. Open the resulting gallery from the job
summary and play both animations.
