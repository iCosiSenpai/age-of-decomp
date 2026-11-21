<script lang="ts">
  // Setup wizard: Welcome → Game file → Checks → Location → Build → Done, with a step bar on top and an overall
  // progress bar while building. Every step works with a controller.
  import { api, PREPARE_STAGES } from '../api';
  import { tr } from '../i18n.svelte';
  import { app } from '../state.svelte';
  import Icon from '../components/Icon.svelte';
  import ProgressBar from '../components/ProgressBar.svelte';
  import Toggle from '../components/Toggle.svelte';
  import Segmented from '../components/Segmented.svelte';
  import type { DiskInfo, NupReport } from '../types';

  let STEPS = $derived([tr('Welcome'), tr('Game file'), tr('Checks'), tr('Location'), tr('Build'), tr('Done')]);
  const NEED_GB = 25;

  // A finished build opens on the last step; one whose game file is no longer where it was opens on the file step.
  let step = $state(app.prepare.running ? 4 : app.launcher?.game.status !== 'complete' ? 0 : app.launcher.game.nup_present ? 5 : 1);
  let nup = $state<string | null>(app.launcher?.game.nup ?? null);
  let stale = $derived(app.launcher?.game.status === 'stale');
  // The game is built and plays, by another engine than this version's: the last step offers the rebuild.
  let engineChanged = $derived(app.launcher?.game.status === 'complete' && (app.launcher?.game.engine_changed ?? false));
  let updateNup = $state<string | null>(app.launcher?.game.update_nup ?? null);
  let dlcNup = $state<string | null>(app.launcher?.game.dlc_nup ?? null);
  let output = $state(app.launcher?.game.output ?? '');
  let found = $state<string[]>([]);
  let report = $state<NupReport | null>(null);
  let checking = $state(false);
  let checkError = $state<string | null>(null);
  let disk = $state<DiskInfo | null>(null);
  let showExtra = $state(false);
  let showLog = $state(false);

  let sys = $derived(app.system);
  let reqs = $derived(
    sys
      ? [
          { ok: sys.avx2, label: tr('Processor with AVX2'), detail: `${sys.cpu} · ${sys.threads} ${tr('threads')}` },
          { ok: sys.vulkan !== null, label: tr('Vulkan 1.3 graphics'), detail: sys.gpus[0]?.name ?? tr('No GPU found') },
          { ok: sys.ram_gb >= 15, warn: sys.ram_gb < 31, label: tr('Memory'), detail: `${sys.ram_gb.toFixed(0)} GB (${tr('32 GB recommended')})` },
        ]
      : [],
  );

  $effect(() => {
    if (step === 1 && !found.length) api.findNups().then((f) => (found = f)).catch(() => {});
    if (step === 3 && output) api.diskInfo(output).then((d) => (disk = d)).catch(() => (disk = null));
  });

  async function pick(kind: 'game' | 'update' | 'dlc') {
    const path = await api.pickFile(kind === 'game' ? tr('Choose your game file (.nup)') : kind === 'update' ? tr('Choose the update file (.nup)') : tr('Choose the DLC file (.nup)'));
    if (!path) return;
    if (kind === 'game') nup = path;
    else if (kind === 'update') updateNup = path;
    else dlcNup = path;
    report = null;
  }

  async function runChecks() {
    if (!nup) return;
    step = 2;
    checking = true;
    checkError = null;
    report = null;
    try {
      report = await api.inspectNup(nup, updateNup);
    } catch (e) {
      checkError = String(e);
    } finally {
      checking = false;
    }
  }

  async function chooseOutput() {
    const dir = await api.pickFolder(tr('Choose where the built game goes'));
    if (dir) {
      output = dir;
      disk = await api.diskInfo(dir).catch(() => null);
    }
  }

  async function startBuild() {
    if (!nup) return;
    try {
      app.launcher = await api.setGame(nup, updateNup, dlcNup, output);
      app.prepare = { running: true, stage: 1, fraction: 0, text: '', elapsed: 0, eta: null, log: [], result: null, error: null };
      step = 4;
      await api.startPrepare();
    } catch (e) {
      app.prepare.running = false;
      app.prepare.result = 'error';
      app.prepare.error = String(e);
    }
  }

  $effect(() => {
    if (app.prepare.result === 'done' && step === 4) step = 5;
  });

  function checkLabel(id: string): string {
    switch (id) {
      case 'title':
        return tr('Game');
      case 'version':
        return tr('Update 3.0.0');
      case 'base':
        return tr('Base game');
      case 'exefs':
        return tr('Program code');
      case 'romfs':
        return tr('Game resources');
      default:
        return id;
    }
  }

  function fmt(s: number | null | undefined): string {
    if (s === null || s === undefined || !isFinite(s)) return '—';
    const m = Math.floor(s / 60);
    const sec = Math.round(s % 60);
    return m ? `${m} min ${sec.toString().padStart(2, '0')} s` : `${sec} s`;
  }
  let pct = $derived(Math.round(app.prepare.fraction * 100));
  let diskOk = $derived(disk ? disk.free_gb >= NEED_GB : true);
</script>

<section class="setup">
  <ol class="stepper" aria-label={tr('Setup steps')}>
    {#each STEPS as label, i (label)}
      <li class:done={i < step} class:current={i === step}>
        <span class="num">{#if i < step}<Icon name="check" size={14} />{:else}{i + 1}{/if}</span>
        <span class="lbl">{label}</span>
      </li>
    {/each}
  </ol>
  <div class="steptrack"><div class="stepfill" style:width="{(step / (STEPS.length - 1)) * 100}%"></div></div>

  <div class="panel card">
    {#if step === 0 && stale}
      <h2>{tr('Rebuild required')}</h2>
      <p class="lead">{tr('The engine was updated. Rebuild to use it. Your existing game files, settings and saves are kept.')}</p>
      <div class="actions">
        {#if nup && app.launcher?.game.nup_present}
          <button class="btn primary" data-autofocus onclick={startBuild}>{tr('Rebuild the game')} <Icon name="refresh" size={18} /></button>
        {/if}
        <button class="btn" onclick={() => (step = 1)}>{tr('Choose game file')} <Icon name="arrow" size={18} /></button>
      </div>
    {:else if step === 0}
      <h2>{tr('Welcome')}</h2>
      <p class="lead">{tr('Age of Decomp turns your own copy of Hyrule Warriors: Age of Imprisonment into a native PC game. Nothing from the game is included: it is built here, from your file, and stays on this PC.')}</p>
      <ul class="facts">
        <li><Icon name="file" size={18} /> {tr('You need your game as a .nup file with update 3.0.0 (NCM 196608), dumped from your own console.')}</li>
        <li><Icon name="disk" size={18} /> {tr('About {gb} GB free on the drive you choose; about 10 minutes on a fast PC.', { gb: NEED_GB })}</li>
        <li><Icon name="bolt" size={18} /> {tr('Your game file is only read, never changed. Keep it where it is: the game reads from it.')}</li>
      </ul>
      <h3 class="sec">{tr('This PC')}</h3>
      <div class="reqs">
        {#each reqs as r (r.label)}
          <div class="req" class:ok={r.ok && !r.warn} class:warn={r.ok && r.warn} class:bad={!r.ok}>
            <Icon name={r.ok ? (r.warn ? 'warn' : 'check') : 'cross'} size={18} />
            <div><div class="rl">{r.label}</div><div class="rd faint">{r.detail}</div></div>
          </div>
        {:else}
          <p class="faint">{tr('Checking this PC…')}</p>
        {/each}
      </div>
      <div class="actions">
        <button class="btn primary" data-autofocus onclick={() => (step = 1)}>{tr('Start')} <Icon name="arrow" size={18} /></button>
      </div>
    {:else if step === 1}
      <h2>{tr('Your game file')}</h2>
      <p class="lead">{tr('Choose the .nup file of your dump. One file with the base game and update 3.0.0 is best; a separate update file works too.')}</p>
      <div class="filebox" class:set={!!nup}>
        <Icon name="file" size={22} />
        <span class="path mono">{nup ?? tr('No file chosen')}</span>
        <button class="btn" data-autofocus onclick={() => pick('game')}>{tr('Choose…')}</button>
      </div>
      {#if found.length}
        <h3 class="sec">{tr('Found on this PC')}</h3>
        <div class="found">
          {#each found as f (f)}
            <button class="btn ghost foundbtn" class:sel={nup === f} onclick={() => { nup = f; report = null; }}><Icon name="file" size={16} /> <span class="mono">{f}</span></button>
          {/each}
        </div>
      {/if}
      <button class="btn ghost extra" onclick={() => (showExtra = !showExtra)}>{showExtra ? '▾' : '▸'} {tr('Separate update or DLC file')}</button>
      {#if showExtra}
        <div class="filebox small">
          <span class="tag">{tr('Update')}</span>
          <span class="path mono">{updateNup ?? tr('Not needed when the game file has the update')}</span>
          <button class="btn" onclick={() => pick('update')}>{tr('Choose…')}</button>
          {#if updateNup}<button class="btn ghost" onclick={() => (updateNup = null)}>{tr('Remove')}</button>{/if}
        </div>
        <div class="filebox small">
          <span class="tag">DLC</span>
          <span class="path mono">{dlcNup ?? tr('Optional')}</span>
          <button class="btn" onclick={() => pick('dlc')}>{tr('Choose…')}</button>
          {#if dlcNup}<button class="btn ghost" onclick={() => (dlcNup = null)}>{tr('Remove')}</button>{/if}
        </div>
      {/if}
      <div class="actions">
        <button class="btn ghost" onclick={() => (step = 0)}><Icon name="back" size={18} /> {tr('Back')}</button>
        <button class="btn primary" disabled={!nup} onclick={runChecks}>{tr('Check the file')} <Icon name="arrow" size={18} /></button>
      </div>
    {:else if step === 2}
      <h2>{tr('Checks')}</h2>
      <p class="lead">{tr('Making sure this is the exact build the engine supports. Only the file list and the program code are read.')}</p>
      {#if checking}
        <div class="checking"><span class="spinner"></span> {tr('Reading your file…')}</div>
      {:else if checkError}
        <div class="result bad"><Icon name="cross" size={20} /> {checkError}</div>
      {:else if report}
        <ul class="checks">
          {#each report.steps as s (s.id)}
            <li class:ok={s.ok} class:bad={!s.ok}>
              <Icon name={s.ok ? 'check' : 'cross'} size={18} />
              <span class="cl">{checkLabel(s.id)}</span>
              <span class="cd faint">{s.detail}</span>
            </li>
          {/each}
        </ul>
        {#if report.ok}
          <div class="result ok"><Icon name="check" size={20} /> {tr('Everything matches. Build ID {id}.', { id: (report.build_id ?? '').slice(0, 16) })}</div>
        {:else}
          <div class="result bad"><Icon name="warn" size={20} /> {tr('This file cannot be used as it is. The game needs the base game and update 3.0.0 (NCM 196608).')}</div>
        {/if}
      {/if}
      <div class="actions">
        <button class="btn ghost" onclick={() => (step = 1)}><Icon name="back" size={18} /> {tr('Back')}</button>
        <button class="btn primary" data-autofocus disabled={!report?.ok} onclick={() => (step = 3)}>{tr('Continue')} <Icon name="arrow" size={18} /></button>
      </div>
    {:else if step === 3}
      <h2>{tr('Where to build')}</h2>
      <p class="lead">{tr('The built game, its resources and your saves go in this folder.')}</p>
      <div class="filebox set">
        <Icon name="folder" size={22} />
        <span class="path mono">{output}</span>
        <button class="btn" onclick={chooseOutput}>{tr('Change…')}</button>
      </div>
      {#if disk}
        <div class="diskline" class:bad={!diskOk}>
          <Icon name={diskOk ? 'check' : 'warn'} size={18} />
          {tr('{free} GB free of {total} GB', { free: disk.free_gb.toFixed(0), total: disk.total_gb.toFixed(0) })}
          {#if !diskOk}· {tr('At least {gb} GB are needed', { gb: NEED_GB })}{/if}
        </div>
      {/if}
      <h3 class="sec">{tr('Options')}</h3>
      <div class="opt">
        <div><div class="rl">{tr('Build speed')}</div><div class="faint">{tr('Automatic uses as many CPU threads as your memory allows.')}</div></div>
        <Segmented
          label={tr('Build speed')}
          options={[{ value: '0', label: tr('Automatic') }, { value: '2', label: '2' }, { value: '4', label: '4' }, { value: '8', label: '8' }]}
          value={String(app.launcher?.ui.jobs ?? 0)}
          onchange={async (v) => (app.launcher = await api.setUi({ jobs: Number(v) }))}
        />
      </div>
      <div class="opt">
        <div><div class="rl">{tr('Experimental frame-rate profile')}</div><div class="faint">{tr('Builds the game so 30, 90 and 120 FPS keep the right speed. Still being tuned: movement can be off.')}</div></div>
        <Toggle label={tr('Experimental frame-rate profile')} checked={app.launcher?.ui.rate_unlock ?? false} onchange={async (v) => (app.launcher = await api.setUi({ rate_unlock: v }))} />
      </div>
      <div class="actions">
        <button class="btn ghost" onclick={() => (step = 2)}><Icon name="back" size={18} /> {tr('Back')}</button>
        <button class="btn primary" data-autofocus disabled={!diskOk || !output} onclick={startBuild}><Icon name="bolt" size={18} /> {tr('Build the game')}</button>
      </div>
    {:else if step === 4}
      <h2>{tr('Building')}</h2>
      <div class="overall">
        <div class="pct">{pct}%</div>
        <div class="bar-wrap">
          <ProgressBar label={tr('Overall progress')} value={app.prepare.fraction} tone={app.prepare.result === 'error' ? 'err' : app.prepare.result === 'done' ? 'ok' : 'focus'} />
          <div class="times faint">
            <span>{app.prepare.text}</span>
            <span>{tr('Elapsed')} {fmt(app.prepare.elapsed)} · {tr('Left')} {fmt(app.prepare.eta)}</span>
          </div>
        </div>
      </div>
      <ol class="stages">
        {#each PREPARE_STAGES as s, i (s.id)}
          {@const n = i + 1}
          <li class:done={n < app.prepare.stage || app.prepare.result === 'done'} class:active={n === app.prepare.stage && app.prepare.running} class:failed={n === app.prepare.stage && app.prepare.result === 'error'}>
            <span class="st">
              {#if n < app.prepare.stage || app.prepare.result === 'done'}<Icon name="check" size={16} />
              {:else if n === app.prepare.stage && app.prepare.running}<span class="spinner small"></span>
              {:else if n === app.prepare.stage && app.prepare.result === 'error'}<Icon name="cross" size={16} />
              {:else}<span class="pending"></span>{/if}
            </span>
            <span>{tr(s.label)}</span>
          </li>
        {/each}
      </ol>
      {#if app.prepare.result === 'error'}
        <div class="result bad"><Icon name="warn" size={20} /> {app.prepare.error ?? tr('The build stopped.')}</div>
      {:else if app.prepare.result === 'cancelled'}
        <div class="result warn"><Icon name="warn" size={20} /> {tr('Stopped. Finished steps are kept: continuing picks up from there.')}</div>
      {/if}
      <button class="btn ghost extra" onclick={() => (showLog = !showLog)}>{showLog ? '▾' : '▸'} {tr('Details')}</button>
      {#if showLog}
        <pre class="log mono">{app.prepare.log.join('\n')}</pre>
      {/if}
      <div class="actions">
        {#if app.prepare.running}
          <button class="btn danger" onclick={() => api.cancelPrepare()}>{tr('Stop')}</button>
        {:else}
          <button class="btn ghost" onclick={() => (step = 3)}><Icon name="back" size={18} /> {tr('Back')}</button>
          <button class="btn primary" data-autofocus onclick={startBuild}><Icon name="refresh" size={18} /> {tr('Continue the build')}</button>
        {/if}
      </div>
    {:else}
      <div class="donebox">
        <div class="seal"><Icon name="check" size={44} /></div>
        <h2>{tr('Ready to play')}</h2>
        <p class="lead">{tr('The game is built for this PC. Settings can be changed any time, here or in game with Esc.')}</p>
        {#if engineChanged}
          <div class="result warn"><Icon name="refresh" size={20} /> {tr('The engine was updated since your game was built. Rebuild to get its fixes: your game files, settings and saves are kept. The game you have still plays.')}</div>
        {/if}
        <div class="actions center">
          {#if engineChanged && nup && app.launcher?.game.nup_present}
            <button class="btn" disabled={app.game.running} onclick={startBuild}><Icon name="refresh" size={18} /> {tr('Rebuild the game')}</button>
          {/if}
          <button class="btn primary" data-autofocus onclick={() => (app.page = 'home')}><Icon name="play" size={18} /> {tr('Go to Play')}</button>
          <button class="btn" onclick={() => (app.page = 'display')}>{tr('Settings')}</button>
          <button class="btn ghost" onclick={() => (step = 1)}>{tr('Build again')}</button>
        </div>
      </div>
    {/if}
  </div>
</section>

<style>
  .setup {
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 980px;
  }
  .stepper {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 6px;
  }
  .stepper li {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-faint);
    font-size: 13.5px;
    font-weight: 600;
  }
  .num {
    width: 26px;
    height: 26px;
    flex: none;
    border-radius: 50%;
    display: grid;
    place-items: center;
    border: 1.5px solid var(--line-strong);
    font-size: 12.5px;
  }
  .stepper li.current {
    color: var(--text);
  }
  .stepper li.current .num {
    border-color: var(--focus);
    color: var(--focus);
    box-shadow: 0 0 0 4px var(--focus-soft);
  }
  .stepper li.done {
    color: var(--text-dim);
  }
  .stepper li.done .num {
    background: var(--ok);
    border-color: var(--ok);
    color: #062014;
  }
  .steptrack {
    height: 4px;
    border-radius: 2px;
    background: var(--surface-3);
    overflow: hidden;
  }
  .stepfill {
    height: 100%;
    background: linear-gradient(90deg, var(--ok), var(--focus));
    transition: width 360ms var(--ease);
  }
  .panel {
    padding: 28px 30px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .lead {
    color: var(--text-dim);
    font-size: 15.5px;
    max-width: 75ch;
  }
  .sec {
    margin-top: 6px;
    font-size: 14px;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .facts {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .facts li {
    display: flex;
    align-items: flex-start;
    gap: 10px;
  }
  .reqs {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
  }
  .req {
    display: flex;
    gap: 10px;
    padding: 12px 14px;
    border-radius: var(--radius-m);
    background: var(--surface-3);
    border: 1px solid var(--line);
  }
  .req.ok {
    color: var(--ok);
  }
  .req.warn {
    color: var(--warn);
  }
  .req.bad {
    color: var(--err);
  }
  .rl {
    color: var(--text);
    font-weight: 620;
  }
  .rd {
    font-size: 12.5px;
  }
  .actions {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    margin-top: 8px;
  }
  .actions.center {
    justify-content: center;
  }
  .filebox {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 16px;
    border-radius: var(--radius-m);
    border: 1.5px dashed var(--line-strong);
    background: var(--surface-1);
  }
  .filebox.set {
    border-style: solid;
    border-color: rgba(63, 198, 255, 0.45);
  }
  .filebox.small {
    padding: 10px 14px;
  }
  .path {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-dim);
  }
  .tag {
    font-size: 12px;
    font-weight: 700;
    padding: 2px 8px;
    border-radius: 6px;
    background: var(--surface-4);
  }
  .found {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .foundbtn {
    justify-content: flex-start;
  }
  .foundbtn.sel {
    border-color: var(--focus);
  }
  .extra {
    align-self: flex-start;
  }
  .checks {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .checks li {
    display: grid;
    grid-template-columns: 24px 170px 1fr;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    border-radius: var(--radius-m);
    background: var(--surface-3);
  }
  .checks li.ok {
    color: var(--ok);
  }
  .checks li.bad {
    color: var(--err);
  }
  .cl {
    color: var(--text);
    font-weight: 600;
  }
  .result {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 16px;
    border-radius: var(--radius-m);
    font-weight: 600;
  }
  .result.ok {
    background: var(--ok-soft);
    color: var(--ok);
  }
  .result.bad {
    background: var(--err-soft);
    color: var(--err);
  }
  .result.warn {
    background: var(--warn-soft);
    color: var(--warn);
  }
  .checking {
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--text-dim);
  }
  .spinner {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 2.5px solid var(--surface-4);
    border-top-color: var(--focus);
    animation: spin 0.8s linear infinite;
    display: inline-block;
  }
  .spinner.small {
    width: 14px;
    height: 14px;
    border-width: 2px;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .diskline {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--ok);
  }
  .diskline.bad {
    color: var(--err);
  }
  .opt {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    padding: 12px 0;
    border-bottom: 1px solid var(--line);
  }
  .overall {
    display: flex;
    align-items: center;
    gap: 20px;
  }
  .pct {
    font-family: var(--font-display);
    font-size: 44px;
    font-weight: 700;
    min-width: 120px;
    color: var(--gold);
  }
  .bar-wrap {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .times {
    display: flex;
    justify-content: space-between;
    font-size: 13px;
  }
  .stages {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 8px 20px;
  }
  .stages li {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-faint);
    padding: 6px 0;
  }
  .stages li.done {
    color: var(--text-dim);
  }
  .stages li.done .st {
    color: var(--ok);
  }
  .stages li.active {
    color: var(--text);
    font-weight: 600;
  }
  .stages li.failed {
    color: var(--err);
  }
  .st {
    width: 18px;
    display: grid;
    place-items: center;
  }
  .pending {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 1.5px solid var(--line-strong);
  }
  .log {
    max-height: 220px;
    overflow: auto;
    padding: 12px;
    border-radius: var(--radius-m);
    background: #07080b;
    border: 1px solid var(--line);
    color: var(--text-dim);
    white-space: pre-wrap;
    user-select: text;
    margin: 0;
  }
  .donebox {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 14px;
    padding: 20px 0;
  }
  .seal {
    width: 92px;
    height: 92px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: #062014;
    background: radial-gradient(circle, #7ef0b0, var(--ok));
    box-shadow: 0 0 40px rgba(79, 209, 139, 0.35);
  }
</style>
