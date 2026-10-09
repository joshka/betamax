# Pull request descriptions

A reviewer should understand the problem and result in ten seconds, then find enough evidence for
a closer read. Open with a concrete use case and what changes for the caller. For example,
“Terminal tests need to click and drag through the application's input path” explains why mouse
commands matter; an implementation inventory does not.

## Make the change easy to skim

- Open with the problem and result in one or two sentences.
- Show the new tape syntax or API usage near the top when users gain a new command or capability.
  A short concrete example should make the change usable without following a specification link.
- Put independent behavior changes in a few short bullets, starting with the useful fact.
- Give evidence and validation distinct places. Use descriptive headings when they help navigation.
- Keep image captions brief and adjacent to the image. Stack screenshots so terminal text stays
  readable.
- Keep actual check results, useful coverage, compatibility implications, and material limitations.

Fit the structure to the change. A small change may need only two paragraphs. Read the opening,
headings, bullet starts, and captions together: can a reviewer understand the change by skimming?

## Explain caller consequences

Explain what a tape author or library caller supplies, what happens during execution, and what
remains their responsibility. Include relevant custom-backend compatibility and failure behavior.
Show the essential usage in the PR, then link the canonical [tape reference](tape-reference.md),
[state contract](state-json.md), or Rustdoc for full rules. Links supplement the example.

Distinguish Betamax behavior, consumer application behavior, fixture inputs, and test coverage.
Adding a consumer tape does not mean its viewer behavior changed. Remove private workspace status,
shipping chronology, implementation inventories, and repeated summaries. Preserve reasons needed
to assess the change.

## Provide evidence that supports the claim

Read the final diff and relevant tests before drafting. Describe preceding work's relevant contract
beside its link so readers need no chat history. Link issues and pull requests by descriptive title.

For visual evidence, preserve the tape and text fixture; do not commit generated PNG or GIF outputs.
Use durable media URLs and inspect the images before publishing. State matched font, theme, fixture,
grid, and canvas settings once per comparison. Caption the action, observable result, and intentional
differences, such as a resized viewport. Label checkpoints from one run as scenarios; reserve
before/after implementation claims for matched runs on the corresponding revisions.

Screenshots show presentation. Exact copied text, cell coordinates, input encoding, and resize
notification need assertions or independent checks. Identify which evidence establishes each claim.
Explain baseline approval and meaningful tolerances when adding image comparisons. See
[terminal testing](terminal-testing.md) and [renderer fidelity](renderer-fidelity.md) for their
respective contracts and coverage limits.

## Report validation accurately

Separate local checks from hosted results and earlier checks from checks on the final revision.
Keep validation brief; avoid a test inventory that crowds out user-facing behavior and examples.
Include the commands or focused checks run and explain meaningful coverage gaps. Replace pending CI
notes once results are known. Rewriting a PR body does not rerun implementation validation.

Do not hard-wrap GitHub paragraphs. Use structured API input or `gh --body-file` for multiline
Markdown. Review once for context, skimming, and claim accuracy, then remove unnecessary detail.
Brevity should reduce reader effort without removing evidence.
