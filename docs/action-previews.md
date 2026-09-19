# PR-built CLI previews

Linux's **Package and smoke** CI job builds the checked-out Betamax CLI with
`cargo build -p betamax --locked`, then passes `target/debug/betamax` to
[`joshka/betamax-action`](https://github.com/joshka/betamax-action)'s `binary` input.
This bypasses the action's release installer. The build log records the executable's version and
SHA-256 for diagnosis; it is not an attestation from a trusted runner.

The action renders `examples/basic.tape` and `examples/ci-playback.tape` as GIF, PNG, MP4 and WebM.
The playback fixture holds two different terminal screens for one and two seconds at normal speed.
`scripts/check-action-playback.py` decodes the action outputs with FFmpeg, requires red then blue
background pixels at 0.5 and 2 seconds,
and checks a 2.9–3.4 second playback duration, allowing frame and encoder rounding around three seconds.
Rendering failures, missing formats, frozen animation and incorrect playback timing fail the job.

Open **Terminal previews → Open gallery** in the job summary for its self-contained HTML gallery.
Individual media and action diagnostics are also run artifacts. GitHub sign-in is required, and
artifacts expire after 14 days. Linux/macOS tests, macOS CLI smoke testing and the specialized
[renderer fidelity PNG/JSON galleries](renderer-fidelity.md) remain in place.

## Known playback-speed issue

During setup, the same fixture with `PlaybackSpeed 2` produced GIF at 3.06 seconds and MP4/WebM
at 3.05 seconds using Betamax 0.1.17, instead of approximately 1.5 seconds. PTY capture currently
records elapsed frame delays without applying the speed multiplier. The normal-speed check does
not cover that separate CLI bug, and this action integration does not change renderer behavior.

Reproduce it with the locally built CLI:

```sh
sed 's/Set PlaybackSpeed 1/Set PlaybackSpeed 2/' examples/ci-playback.tape \
  | target/debug/betamax run --quiet --output target/playback-speed-2.mp4 -
ffprobe -v error -show_entries format=duration -of default=nw=1 target/playback-speed-2.mp4
```

## Trust boundary

The PR build and render job runs on a disposable GitHub-hosted runner with `contents: read`,
`persist-credentials: false` and no secrets. PR code, build scripts, tapes and the resulting binary
can compromise that entire runner. The action's path checks, output checks and playback assertions
are diagnostics, not a sandbox or a security boundary. Never add a PAT or publication credential to
any later step of that job.

The existing **Renderer gallery comment** workflow runs after CI on a fresh runner. Its custom
reporter comes from the default branch; the Betamax reporter uses the same reviewed commit pin as
the rendering action. It checks source workflow, run, current PR head and artifact metadata, then
publishes gallery links using `GITHUB_TOKEN` with `actions: read` and `pull-requests: write`.
It never checks out PR code, executes the PR binary, restores PR caches or downloads artifact
contents. Generated galleries remain untrusted downloadable artifacts.

Native attachments are disabled. Enabling them later requires a Betamax-scoped secret and a review
of the trusted reporter's independent validation of downloaded media. Validation in the compromised
render job is insufficient. A token scoped to `joshka/betamax-action` must not be reused here.

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
branch. After merging, use a subsequent PR run to verify the **Terminal previews** comment and
rerun the reporting workflow to confirm it updates the same comment. Before that activation, the
PR run's gallery and media artifacts provide live rendering evidence, but do not prove the new
reporter step ran in this repository.
