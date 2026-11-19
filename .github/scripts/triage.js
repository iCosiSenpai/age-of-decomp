// Issue and pull request triage for Age of Decomp (run by .github/workflows/triage.yml through actions/github-script).
// Issues: labels from the issue form (RecompLauncher, Windows, Linux, Steam Deck), one reminder when game files, keys or
// saves are attached, and one request for the version or log a bug report needs. Pull requests: labels from the
// changed paths. It never reads or runs code of a pull request: only the GitHub API is used.

const LABELS = {
  crash: ['b60205', 'Crashes, freezes or does not start'],
  graphics: ['5319e7', 'Picture, resolution, colour or shader problems'],
  controllers: ['0e8a16', 'Controllers, rumble, motion and button layouts'],
  bug: ['d73a4a', "Something isn't working"],
  enhancement: ['a2eeef', 'New feature or request'],
  recomplauncher: ['1d76db', 'Started from or about RecompLauncher'],
  windows: ['0052cc', 'Windows'],
  linux: ['fbca04', 'Linux'],
  'steam-deck': ['c5def5', 'Steam Deck'],
  'needs-info': ['ffb547', 'Waiting for the version or a log from the reporter'],
  'game-files': ['ff5a4f', 'Game files, keys or saves were posted: remove them'],
  engine: ['3fc6ff', 'Engine patches and runtime'],
  launcher: ['e7b75a', 'The Age of Decomp launcher'],
  nup: ['bfdadc', 'The NUP reader crate'],
  docs: ['0075ca', 'Documentation'],
  ci: ['ededed', 'Workflows and repository tools'],
};

// Paths of a pull request → labels (first match per file; several files can add several labels).
const PATH_LABELS = [
  [/^engine\//, 'engine'],
  [/^apps\/launcher\/src\/lib\/settings\/catalog\.json$/, 'recomplauncher'],
  [/^packaging\//, 'recomplauncher'],
  [/^apps\/launcher\//, 'launcher'],
  [/^apps\/nintendo-bridge\//, 'controllers'],
  [/^crates\/aod-nup\//, 'nup'],
  [/^(docs\/|README\.md$|THIRD_PARTY_NOTICES\.md$)/, 'docs'],
  [/^(\.github\/|tools\/)/, 'ci'],
];

const NO_RESPONSE = '_No response_';
const MARK = (kind) => `<!-- aod-triage:${kind} -->`;

// "### Label\n\nvalue" sections of an issue form, by label.
function sections(body) {
  const out = {};
  const parts = String(body || '').split(/^### /m).slice(1);
  for (const part of parts) {
    const nl = part.indexOf('\n');
    const label = (nl < 0 ? part : part.slice(0, nl)).trim();
    const value = (nl < 0 ? '' : part.slice(nl + 1)).trim();
    out[label] = value === NO_RESPONSE ? '' : value;
  }
  return out;
}

function field(s, ...names) {
  for (const [label, value] of Object.entries(s)) {
    if (names.some((n) => label.toLowerCase().startsWith(n.toLowerCase()))) return value;
  }
  return undefined;
}

// What an issue body says, as labels and notes (pure: unit-tested by .github/scripts/triage.test.js).
function readIssue(body, labels) {
  const s = sections(body);
  const text = String(body || '');
  const add = new Set();
  const started = field(s, 'Started from') || '';
  if (/recomplauncher/i.test(started) || /recomplauncher/i.test(text)) add.add('recomplauncher');
  const system = [field(s, 'System', 'Operating system', 'OS') || '', started].join(' ');
  if (/steam ?deck|steamos/i.test(system)) add.add('steam-deck');
  if (/linux|steam ?deck|steamos|ubuntu|fedora|arch|mint|bazzite|nobara/i.test(system)) add.add('linux');
  if (/windows|win ?1[01]/i.test(system)) add.add('windows');

  // Links or attachments of game files, keys or saves (never needed; the templates ask not to post them).
  const files = [];
  if (/\.(nup|nsp|nsz|xci|xcz|nca|nso|nro)\b/i.test(text)) files.push('game files');
  if (/\b(prod|title|dev)\.keys\b/i.test(text)) files.push('keys');
  if (/user-attachments\/files\/[^\s)]*sav[^\s)]*\.(zip|7z|rar)/i.test(text) || /\bsave[^\s)]*\.(zip|7z|rar)\b/i.test(text)) {
    files.push('saves');
  }

  // A bug report with neither a version nor a log can't be followed up.
  const isReport = ['crash', 'graphics', 'controllers', 'bug'].some((l) => labels.includes(l));
  const missing = [];
  if (isReport) {
    const version = field(s, 'Age of Decomp version');
    if (version !== undefined && !/v?\d+\.\d+\.\d+/.test(version)) missing.push('the Age of Decomp version (for example v0.1.2)');
    const logs = field(s, 'Logs', 'Game log');
    const attached = /github\.com\/user-attachments\/(files|assets)\//.test(text);
    if (logs !== undefined && !logs && !attached) missing.push('the newest log (`game-*.log` from the game folder\'s `logs`, or `prepare-*.log` for setup)');
  }
  return { add: [...add], files, missing };
}

async function ensureLabels(github, owner, repo, names) {
  for (const name of names) {
    const [color, description] = LABELS[name] || ['ededed', ''];
    try {
      await github.rest.issues.getLabel({ owner, repo, name });
    } catch (e) {
      if (e.status !== 404) throw e;
      try {
        await github.rest.issues.createLabel({ owner, repo, name, color, description });
      } catch (e2) {
        if (e2.status !== 422) throw e2;   // made by a run at the same time
      }
    }
  }
}

async function commentOnce(github, owner, repo, number, kind, text) {
  const comments = await github.paginate(github.rest.issues.listComments, { owner, repo, issue_number: number, per_page: 100 });
  if (comments.some((c) => (c.body || '').includes(MARK(kind)))) return false;
  await github.rest.issues.createComment({ owner, repo, issue_number: number, body: `${MARK(kind)}\n${text}` });
  return true;
}

async function triageIssue({ github, context, core }) {
  const { owner, repo } = context.repo;
  const issue = context.payload.issue;
  if (!issue || issue.pull_request) return;
  const labels = (issue.labels || []).map((l) => (typeof l === 'string' ? l : l.name));
  const r = readIssue(issue.body, labels);
  const add = [...r.add];
  if (r.files.length) add.push('game-files');
  if (r.missing.length) add.push('needs-info');
  const fresh = add.filter((l) => !labels.includes(l));
  if (fresh.length) {
    await ensureLabels(github, owner, repo, fresh);
    await github.rest.issues.addLabels({ owner, repo, issue_number: issue.number, labels: fresh });
  }
  if (r.files.length) {
    await commentOnce(github, owner, repo, issue.number, 'files',
      `Thanks for the report. Please remove the ${r.files.join(', ')} from this issue (edit it and delete the links): ` +
      'Age of Decomp never needs them, and game files, keys and saves must not be shared here. Logs are fine.');
  }
  if (r.missing.length) {
    await commentOnce(github, owner, repo, issue.number, 'needs-info',
      `Thanks for the report. To look into it, please add ${r.missing.join(' and ')}: edit the issue and drag the file in.`);
  } else if (labels.includes('needs-info') && context.payload.action === 'edited') {
    try {
      await github.rest.issues.removeLabel({ owner, repo, issue_number: issue.number, name: 'needs-info' });
    } catch (e) {
      if (e.status !== 404) throw e;
    }
  }
  core.info(`issue #${issue.number}: added ${fresh.join(', ') || 'no labels'}`);
}

function pathLabels(files) {
  const out = new Set();
  for (const f of files) {
    for (const [re, label] of PATH_LABELS) {
      if (re.test(f)) { out.add(label); break; }
    }
  }
  return [...out];
}

async function triagePull({ github, context, core }) {
  const { owner, repo } = context.repo;
  const pr = context.payload.pull_request;
  if (!pr) return;
  const files = await github.paginate(github.rest.pulls.listFiles, { owner, repo, pull_number: pr.number, per_page: 100 });
  const have = (pr.labels || []).map((l) => l.name);
  const fresh = pathLabels(files.map((f) => f.filename)).filter((l) => !have.includes(l));
  if (fresh.length) {
    await ensureLabels(github, owner, repo, fresh);
    await github.rest.issues.addLabels({ owner, repo, issue_number: pr.number, labels: fresh });
  }
  core.info(`pull request #${pr.number}: added ${fresh.join(', ') || 'no labels'}`);
}

async function run(args) {
  if (args.context.eventName === 'issues') return triageIssue(args);
  if (args.context.eventName === 'pull_request_target') return triagePull(args);
}

module.exports = run;
module.exports.readIssue = readIssue;
module.exports.pathLabels = pathLabels;
module.exports.sections = sections;
module.exports.ensureLabels = ensureLabels;
module.exports.LABELS = LABELS;
