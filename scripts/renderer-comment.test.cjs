const assert = require('node:assert/strict');
const test = require('node:test');
const publish = require('./renderer-comment.cjs');

const marker = '<!-- betamax-renderer-galleries -->';
const botComment = {
  id: 99,
  user: { login: 'github-actions[bot]' },
  body: `${marker}\n<!-- run:9 attempt:1 -->`,
};

async function run({ comments = [], artifacts = [], head = {}, state = 'open', fork = false } = {}) {
  const writes = [];
  const api = {
    listWorkflowRunArtifacts: 'artifacts',
    listPullRequestsAssociatedWithCommit: 'pulls',
    listComments: 'comments',
    get: async () => ({ data: {
      state,
      head: { sha: 'abc', repo: { id: 42 }, ref: 'feature', ...head },
      base: { repo: { full_name: 'owner/repo' } },
    } }),
    createComment: async args => writes.push({ method: 'create', ...args }),
    updateComment: async args => writes.push({ method: 'update', ...args }),
  };
  const github = {
    rest: { actions: api, repos: api, pulls: api, issues: api },
    paginate: async method => ({ artifacts, pulls: [{ number: 1 }], comments })[method],
  };
  const context = {
    repo: { owner: 'owner', repo: 'repo' },
    payload: { workflow_run: {
      id: 10, run_number: 5, run_attempt: 2, head_sha: 'abc',
      head_repository: { id: 42 }, head_branch: 'feature',
      html_url: 'https://github.com/owner/repo/actions/runs/10',
      conclusion: 'failure', pull_requests: fork ? [] : [{ number: 1 }],
    } },
  };
  await publish({ github, context });
  return writes;
}

test('fork lookup creates a comment with available links and explicit partial output', async () => {
  const [comment] = await run({ fork: true, artifacts: [
    { id: 1, name: 'renderer-fidelity-ubuntu-latest.html' },
    { id: 2, name: 'renderer-fidelity-macos-14.html', expired: true },
    { id: 3, name: '[untrusted](https://example.com)' },
  ] });
  assert.equal(comment.method, 'create');
  assert.equal(comment.issue_number, 1);
  assert.match(comment.body, /View Linux fixtures.*\/artifacts\/1/);
  assert.match(comment.body, /macOS gallery unavailable/);
  assert.match(comment.body, /\*\*failure\*\*/);
  assert.doesNotMatch(comment.body, /untrusted/);
});

test('updates the existing bot comment instead of duplicating it', async () => {
  const [comment] = await run({ comments: [botComment] });
  assert.equal(comment.method, 'update');
  assert.equal(comment.comment_id, 99);
  assert.match(comment.body, /run:10 attempt:2/);
});

test('does not edit a user comment containing the marker', async () => {
  const [comment] = await run({ comments: [{ ...botComment, user: { login: 'someone' } }] });
  assert.equal(comment.method, 'create');
});

for (const [name, options] of [
  ['changed head', { head: { sha: 'newer' } }],
  ['different source repository', { head: { repo: { id: 43 } } }],
  ['different source branch', { head: { ref: 'other' } }],
  ['closed PR', { state: 'closed' }],
]) {
  test(`skips ${name}`, async () => assert.deepEqual(await run(options), []));
}

for (const stamp of ['run:11 attempt:1', 'run:10 attempt:3']) {
  test(`preserves newer results: ${stamp}`, async () => {
    const comments = [{ ...botComment, body: `${marker}\n<!-- ${stamp} -->` }];
    assert.deepEqual(await run({ comments }), []);
  });
}
