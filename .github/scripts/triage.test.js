// node --test .github/scripts/triage.test.js — the pure parts of the triage and guide scripts, no GitHub access.
const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { readIssue, pathLabels, sections } = require('./triage.js');
const { issueBody, reviewedSha } = require('./recomplauncher-guide.js');

const crash = (fields) => Object.entries(fields).map(([k, v]) => `### ${k}\n\n${v}`).join('\n\n');

test('issue form sections, "No response" as empty', () => {
  const s = sections(crash({ 'Age of Decomp version': 'v0.1.1', Logs: '_No response_' }));
  assert.deepStrictEqual(s, { 'Age of Decomp version': 'v0.1.1', Logs: '' });
});

test('RecompLauncher and Windows from the form', () => {
  const r = readIssue(crash({
    'Age of Decomp version': 'v0.1.1', 'Started from': 'RecompLauncher',
    'System: OS, CPU, GPU and driver, RAM': 'Windows 11 Pro, Ryzen 7 7700, RX 9070 XT, 31 GB',
    Logs: 'game log here',
  }), ['crash']);
  assert.deepStrictEqual(r.add.sort(), ['recomplauncher', 'windows']);
  assert.deepStrictEqual(r.files, []);
  assert.deepStrictEqual(r.missing, []);
});

test('Steam Deck counts as Linux too', () => {
  const r = readIssue(crash({ 'Age of Decomp version': 'v0.1.2', 'System: OS, CPU, GPU and driver, RAM': 'Steam Deck OLED', Logs: 'x' }), ['bug']);
  assert.deepStrictEqual(r.add.sort(), ['linux', 'steam-deck']);
});

test('a crash report without version or log asks for both', () => {
  const r = readIssue(crash({ 'Age of Decomp version': 'latest', Logs: '_No response_', 'What happened': 'It closes.' }), ['crash']);
  assert.strictEqual(r.missing.length, 2);
});

test('an attached file counts as a log', () => {
  const r = readIssue(crash({ 'Age of Decomp version': 'v0.1.2', Logs: '_No response_',
    'What happened': 'see [game-1.log](https://github.com/user-attachments/files/123/game-1.log)' }), ['crash']);
  assert.deepStrictEqual(r.missing, []);
});

test('feature requests are never asked for logs', () => {
  const r = readIssue(crash({ 'What would you like': 'More options' }), ['enhancement']);
  assert.deepStrictEqual(r.missing, []);
});

test('game files, keys and save archives are noticed', () => {
  const r = readIssue('Here: [game.nsp](https://x/game.nsp), my prod.keys and ' +
    '[Hyrule.save.zip](https://github.com/user-attachments/files/35083954/Hyrule.Warriors.Age.of.Calamity.save.zip)', ['enhancement']);
  assert.deepStrictEqual(r.files, ['game files', 'keys', 'saves']);
});

test('a log file name is not a game file', () => {
  const r = readIssue('[game-1795.log](https://github.com/user-attachments/files/1/game-1795.log) and the nup reader', ['bug']);
  assert.deepStrictEqual(r.files, []);
});

test('pull request labels from paths', () => {
  assert.deepStrictEqual(pathLabels([
    'engine/source/runtime/input.c', 'apps/launcher/src-tauri/src/game.rs', 'apps/launcher/src/lib/settings/catalog.json',
    'packaging/recomplauncher.json', 'docs/SETTINGS.md', '.github/workflows/ci.yml', 'crates/aod-nup/src/lib.rs',
  ]).sort(), ['ci', 'docs', 'engine', 'launcher', 'nup', 'recomplauncher']);
});

test('the guide file names a full commit id', () => {
  const sha = reviewedSha(path.join(__dirname, '..', '..', 'packaging', 'recomplauncher-guide.txt'));
  assert.match(sha, /^[0-9a-f]{40}$/);
  const bad = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'aod-guide-')), 'guide.txt');
  fs.writeFileSync(bad, '# no id\n');
  assert.throws(() => reviewedSha(bad));
});

test('the guide issue lists each change once', () => {
  const body = issueBody('a'.repeat(40), [
    { sha: 'b'.repeat(40), date: '2026-11-20T10:00:00Z', title: 'Saves: say where', url: 'https://example.invalid/b', files: ['docs/DEVELOPERS.md'] },
  ]);
  assert.match(body, /2026-11-20 \[bbbbbbb\]\(https:\/\/example\.invalid\/b\) Saves: say where \(docs\/DEVELOPERS\.md\)/);
  assert.match(body, /packaging\/recomplauncher-guide\.txt/);
});
