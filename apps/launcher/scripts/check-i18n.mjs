// Checks the Italian and Simplified Chinese dictionaries against the UI: every literal tr('...') in src and every English
// label/help in src/lib/settings/catalog.json must have a translation in both, an English key must not get two different
// translations, the two dictionaries hold the same keys, and every {placeholder} of a key is in its translations.
// Run by `pnpm check`. Adapted from the maintainer's Deluxe8 launcher.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const root = new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1');
const src = join(root, 'src');
const dictFile = join(src, 'lib', 'i18n', 'it.ts');
const zhFile = join(src, 'lib', 'i18n', 'zh.ts');

function walk(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? walk(path) : [path];
  });
}

// A dictionary: one `"english": "translation",` entry per line (JSON strings).
const problems = [];
function dictionary(file, name) {
  const map = new Map();
  for (const line of readFileSync(file, 'utf8').split('\n')) {
    const m = line.match(/^\s*("(?:[^"\\]|\\.)*")\s*:\s*("(?:[^"\\]|\\.)*"),?\s*$/);
    if (!m) continue;
    const [en, tl] = [JSON.parse(m[1]), JSON.parse(m[2])];
    const seen = map.get(en);
    if (seen !== undefined && seen !== tl) problems.push(`${name}: "${en}" has two translations: "${seen}" and "${tl}"`);
    map.set(en, tl);
  }
  return map;
}
const italian = dictionary(dictFile, 'Italian');
const chinese = dictionary(zhFile, 'Chinese');
const placeholders = (t) => [...t.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',');
for (const [name, map, other] of [['Chinese', chinese, italian], ['Italian', italian, chinese]]) {
  for (const [en, tl] of map) {
    if (!other.has(en)) problems.push(`${name} has "${en}", the other dictionary does not`);
    if (placeholders(en) !== placeholders(tl)) problems.push(`${name}: "${en}" and "${tl}" have different {placeholders}`);
  }
}

const call = /\btr\(\s*(?:'((?:[^'\\]|\\.)*)'|"((?:[^"\\]|\\.)*)"|(`))/g;
let used = 0;
const usedTexts = new Set();   // every English string the UI shows, for --unused
for (const file of walk(src).filter((f) => /\.(svelte|ts)$/.test(f) && f !== dictFile)) {
  const text = readFileSync(file, 'utf8');
  for (const m of text.matchAll(call)) {
    const where = `${relative(root, file)}:${text.slice(0, m.index).split('\n').length}`;
    if (m[3]) {
      problems.push(`${where}: tr() with a template literal; use tr('... {name} ...', { name })`);
      continue;
    }
    const en = (m[1] ?? m[2]).replace(/\\(['"\\])/g, '$1');
    used++;
    usedTexts.add(en);
    if (!italian.has(en)) problems.push(`${where}: no Italian for "${en}"`);
    if (!chinese.has(en)) problems.push(`${where}: no Chinese for "${en}"`);
  }
}

// Catalogue labels, help texts, group labels and choice labels (shown through trc()).
const catalog = JSON.parse(readFileSync(join(src, 'lib', 'settings', 'catalog.json'), 'utf8'));
const texts = new Set();
for (const g of catalog.groups) texts.add(g.label);
for (const s of catalog.settings) {
  texts.add(s.label);
  texts.add(s.help);
  for (const c of s.choices ?? []) if (/[a-z]{3,}/.test(c.label) || !/^[\d×x .]+(p|FPS|%)?$/i.test(c.label)) texts.add(c.label);
}
// Page intros in SettingsPage.svelte go through trc() too.
const pageSrc = readFileSync(join(src, 'lib', 'pages', 'SettingsPage.svelte'), 'utf8');
for (const m of pageSrc.matchAll(/^\s+\w+: '((?:[^'\\]|\\.)*)',$/gm)) texts.add(m[1].replace(/\\'/g, "'"));
// Labels passed to tr() through a variable: NAV (state.svelte.ts) and PREPARE_STAGES (api.ts).
for (const f of ['state.svelte.ts', 'api.ts']) {
  const t = readFileSync(join(src, 'lib', f), 'utf8');
  for (const m of t.matchAll(/label: '((?:[^'\\]|\\.)*)'/g)) texts.add(m[1]);
}
// Native language names are shown as they are.
const native = new Set(['Italiano', 'Français', 'Français (Canada)', 'Deutsch', 'Español', 'Español (Latinoamérica)', 'Nederlands',
  'Português', 'Português (Brasil)', 'Русский', '日本語', '한국어', '简体中文', '繁體中文', 'English (US)', 'English (UK)', 'English']);
for (const t of texts) {
  used++;
  usedTexts.add(t);
  if (!native.has(t) && !italian.has(t)) problems.push(`catalog: no Italian for "${t}"`);
  if (!native.has(t) && !chinese.has(t)) problems.push(`catalog: no Chinese for "${t}"`);
}

// `node scripts/check-i18n.mjs --unused`: the dictionary entries no string of the UI uses any more, one per line.
if (process.argv.includes('--unused')) {
  for (const en of italian.keys()) if (!usedTexts.has(en)) console.log(JSON.stringify(en));
  process.exit(0);
}

if (problems.length) {
  console.error(problems.join('\n'));
  console.error(`\ni18n: ${problems.length} problem(s)`);
  process.exit(1);
}
console.log(`i18n: ${used} strings in the UI, ${italian.size} Italian and ${chinese.size} Chinese translations, all covered`);
