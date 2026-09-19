// Executed from the default-branch commit, never from the triggering PR.
const COMMENT_MARKER = '<!-- betamax-renderer-galleries -->';
const GALLERIES = [
  { label: 'Linux', name: 'renderer-fidelity-ubuntu-latest.html' },
  { label: 'macOS', name: 'renderer-fidelity-macos-14.html' },
];

function galleryLink(runUrl, artifacts, { label, name }) {
  const available = artifacts.filter(artifact => artifact.name === name && !artifact.expired);
  const latest = available.sort((a, b) => b.id - a.id)[0];
  if (!latest) return `- ${label} gallery unavailable: no retained gallery was uploaded.`;
  return `- [View ${label} fixtures](${runUrl}/artifacts/${latest.id})`;
}

function commentBody(run, artifacts) {
  const runLink = `[CI run ${run.run_number}, attempt ${run.run_attempt}](${run.html_url})`;
  const revision = `commit \`${run.head_sha.slice(0, 12)}\``;
  return [
    COMMENT_MARKER,
    '### Renderer fixtures',
    '',
    ...GALLERIES.map(gallery => galleryLink(run.html_url, artifacts, gallery)),
    '',
    `${runLink} · **${run.conclusion}** · ${revision}`,
    '',
    'Failed or cancelled runs may contain partial checkpoints. ' +
      'Images are captured output, not a guarantee of rendering correctness.',
    '',
    'Artifacts require GitHub sign-in and expire with repository retention.',
    `<!-- run:${run.id} attempt:${run.run_attempt} -->`,
  ].join('\n');
}

function matchesPullRequest(pr, run, repo) {
  return pr.state === 'open' &&
    pr.head.sha === run.head_sha &&
    pr.head.repo?.id === run.head_repository?.id &&
    pr.head.ref === run.head_branch &&
    pr.base.repo.full_name === `${repo.owner}/${repo.repo}`;
}

function isReporterComment(comment) {
  return comment.user?.login === 'github-actions[bot]' &&
    comment.body?.startsWith(COMMENT_MARKER);
}

function hasNewerResults(comment, run) {
  const stamp = comment?.body.match(/<!-- run:(\d+) attempt:(\d+) -->/);
  if (!stamp) return false;
  const runId = Number(stamp[1]);
  const attempt = Number(stamp[2]);
  return runId > run.id || (runId === run.id && attempt > run.run_attempt);
}

async function associatedPullRequests(github, repo, run) {
  if (run.pull_requests?.length) return run.pull_requests;
  // Fork runs can omit PR associations from the event payload.
  return github.paginate(github.rest.repos.listPullRequestsAssociatedWithCommit, {
    ...repo, commit_sha: run.head_sha, per_page: 100,
  });
}

async function updatePullRequestComment(github, repo, number, run, body) {
  // Re-read the PR: it may have changed or closed since CI started.
  const { data: pr } = await github.rest.pulls.get({ ...repo, pull_number: number });
  if (!matchesPullRequest(pr, run, repo)) return;

  const comments = await github.paginate(github.rest.issues.listComments, {
    ...repo, issue_number: number, per_page: 100,
  });
  const existing = comments.find(isReporterComment);
  // Runs can finish out of order. Never replace newer evidence with older links.
  if (hasNewerResults(existing, run)) return;

  if (existing) {
    await github.rest.issues.updateComment({ ...repo, comment_id: existing.id, body });
  } else {
    await github.rest.issues.createComment({ ...repo, issue_number: number, body });
  }
}

async function publishRendererComment({ github, context }) {
  const run = context.payload.workflow_run;
  const repo = context.repo;
  const artifacts = await github.paginate(github.rest.actions.listWorkflowRunArtifacts, {
    ...repo, run_id: run.id, per_page: 100,
  });
  const body = commentBody(run, artifacts);
  const pulls = await associatedPullRequests(github, repo, run);
  for (const { number } of pulls) {
    await updatePullRequestComment(github, repo, number, run, body);
  }
}

module.exports = publishRendererComment;
