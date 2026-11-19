// Watches RecompLauncher's developer guide (run by .github/workflows/recomplauncher.yml through actions/github-script).
// packaging/recomplauncher-guide.txt holds the RecompLauncher commit whose guide Age of Decomp was last checked against.
// When the guide, the manifest schema or the setup inputs page changed after it, one open issue (label
// recomplauncher-guide) lists those commits; once the file names a newer commit, the issue is closed.

const fs = require('fs');

const UPSTREAM = { owner: 'elripalda', repo: 'RecompLauncher' };
const WATCHED = ['docs/DEVELOPERS.md', 'docs/recomplauncher.schema.json', 'docs/SETUP-INPUTS.md'];
const LABEL = 'recomplauncher-guide';
const TITLE = "RecompLauncher's developer guide changed";

function reviewedSha(path) {
  const line = fs.readFileSync(path, 'utf8').split('\n').map((l) => l.trim()).find((l) => /^[0-9a-f]{40}$/.test(l));
  if (!line) throw new Error(`${path} has no 40-character commit id`);
  return line;
}

// Commits that touched a watched file after `since` (an ISO date), newest first, without the reviewed one itself.
async function changesAfter(github, reviewed, since) {
  const seen = new Map();
  for (const path of WATCHED) {
    const commits = await github.paginate(github.rest.repos.listCommits, { ...UPSTREAM, path, since, per_page: 100 });
    for (const c of commits) {
      if (c.sha === reviewed) continue;
      const entry = seen.get(c.sha) || { sha: c.sha, date: c.commit.committer.date, title: c.commit.message.split('\n')[0], url: c.html_url, files: [] };
      entry.files.push(path);
      seen.set(c.sha, entry);
    }
  }
  return [...seen.values()].sort((a, b) => b.date.localeCompare(a.date));
}

function issueBody(reviewed, changes) {
  const rows = changes.map((c) => `- ${c.date.slice(0, 10)} [${c.sha.slice(0, 7)}](${c.url}) ${c.title} (${c.files.join(', ')})`);
  return [
    `RecompLauncher's developer guide changed after [${reviewed.slice(0, 7)}](https://github.com/${UPSTREAM.owner}/${UPSTREAM.repo}/commit/${reviewed}), the commit Age of Decomp was last checked against:`,
    '',
    ...rows,
    '',
    'To do: read these changes, adapt the manifest (`tools/release.py manifest`) or the launcher where needed, run',
    '`py tools/recomplauncher_check.py` (level 3, no errors), then put the newest commit id in',
    '`packaging/recomplauncher-guide.txt`. This issue closes itself after that.',
  ].join('\n');
}

async function run({ github, context, core }) {
  const { owner, repo } = context.repo;
  const reviewed = reviewedSha('packaging/recomplauncher-guide.txt');
  const base = await github.rest.repos.getCommit({ ...UPSTREAM, ref: reviewed });
  const changes = await changesAfter(github, reviewed, base.data.commit.committer.date);
  const open = (await github.rest.issues.listForRepo({ owner, repo, state: 'open', labels: LABEL, per_page: 10 })).data;
  if (!changes.length) {
    for (const issue of open) {
      await github.rest.issues.createComment({ owner, repo, issue_number: issue.number,
        body: `Checked against ${reviewed.slice(0, 7)}: nothing newer in the guide.` });
      await github.rest.issues.update({ owner, repo, issue_number: issue.number, state: 'closed' });
    }
    core.info(`guide unchanged since ${reviewed}`);
    return;
  }
  const body = issueBody(reviewed, changes);
  if (open.length) {
    if (open[0].body !== body) await github.rest.issues.update({ owner, repo, issue_number: open[0].number, body });
    core.info(`issue #${open[0].number} lists ${changes.length} change(s)`);
  } else {
    try {
      await github.rest.issues.getLabel({ owner, repo, name: LABEL });
    } catch (e) {
      if (e.status !== 404) throw e;
      await github.rest.issues.createLabel({ owner, repo, name: LABEL, color: '1d76db', description: "RecompLauncher's developer guide changed" });
    }
    const made = await github.rest.issues.create({ owner, repo, title: TITLE, body, labels: [LABEL, 'recomplauncher'] });
    core.info(`opened issue #${made.data.number} for ${changes.length} change(s)`);
  }
}

module.exports = run;
module.exports.issueBody = issueBody;
module.exports.reviewedSha = reviewedSha;
