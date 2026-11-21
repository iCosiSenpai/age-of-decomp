<script lang="ts">
  import { api } from '../api';
  import { tr } from '../i18n.svelte';
  import { app, catalog } from '../state.svelte';
  import Logo from '../components/Logo.svelte';
  import Icon from '../components/Icon.svelte';
  import Glyph from '../components/Glyph.svelte';
  import SettingRow from '../components/SettingRow.svelte';

  // The build's state: none (not set up), incomplete (the build was stopped), stale (the engine was updated and the
  // game must be built again before it plays), complete. A complete build made by another engine than this
  // version's still plays: a rebuild is offered.
  let status = $derived(app.launcher?.game.status ?? 'none');
  let built = $derived(status === 'complete');
  let engineChanged = $derived(built && (app.launcher?.game.engine_changed ?? false));
  let nupOk = $derived(app.launcher?.game.nup_present ?? false);
  // The graphics card the game will use: the one chosen in Display, else a dedicated card before an integrated one.
  let gpu = $derived.by(() => {
    const gpus = app.system?.gpus ?? [];
    const chosen = app.settings.gpu_index;
    return (chosen ? gpus.find((g) => String(g.index) === chosen) : undefined) ?? gpus.find((g) => g.discrete) ?? gpus[0] ?? null;
  });
  let quick = $derived(catalog.settings.filter((s) => ['output_res', 'fps_limit', 'fullscreen'].includes(s.key)));
  let starting = $state(false);

  async function play() {
    if (!built || app.game.running || starting) return;
    starting = true;
    try {
      await api.play();
    } catch (e) {
      app.toast(String(e), 'err');
    } finally {
      setTimeout(() => (starting = false), 1500);
    }
  }
</script>

<section class="home">
  <div class="hero">
    <div class="emblem"><Logo size={132} /></div>
    <div class="title">
      <h1>Age of Decomp</h1>
      <p class="sub">{tr('Hyrule Warriors: Age of Imprisonment, recompiled for your PC from your own copy.')}</p>
      <div class="chips">
        {#if built && nupOk}
          <span class="chip ok"><Icon name="check" size={15} /> {tr('Ready to play')}</span>
        {:else if built}
          <span class="chip err"><Icon name="warn" size={15} /> {tr('Your game file has moved')}</span>
        {:else if status === 'stale'}
          <span class="chip warn"><Icon name="warn" size={15} /> {tr('Rebuild required')}</span>
        {:else if status === 'incomplete'}
          <span class="chip warn"><Icon name="warn" size={15} /> {tr('Setup not finished')}</span>
        {:else}
          <span class="chip warn"><Icon name="warn" size={15} /> {tr('Setup needed')}</span>
        {/if}
        {#if engineChanged}
          <span class="chip warn"><Icon name="refresh" size={15} /> {tr('Rebuild available')}</span>
        {/if}
        <span class="chip"><Icon name="display" size={15} /> {(app.settings.output_res ?? 'auto') === 'auto' ? tr('Automatic') : app.settings.output_res} · {app.settings.fps_limit ?? '60'} FPS</span>
        {#if gpu}<span class="chip"><Icon name="chip" size={15} /> {gpu.name}</span>{/if}
      </div>
    </div>
  </div>

  <div class="play-row">
    {#if built}
      <button class="btn primary play" data-autofocus disabled={app.game.running || starting || !nupOk} onclick={play}>
        <Icon name="play" size={22} />
        {app.game.running ? tr('Playing…') : starting ? tr('Starting…') : tr('Play')}
      </button>
      <span class="hint faint"><Glyph button="start" family={app.hintPad} /> {tr('from anywhere in the launcher')}</span>
    {:else if status === 'stale'}
      <button class="btn primary play" data-autofocus onclick={() => (app.page = 'setup')}>
        <Icon name="refresh" size={20} />
        {tr('Rebuild the game')}
      </button>
      <span class="hint faint">{tr('The engine was updated. Rebuild to use it. Your existing game files, settings and saves are kept.')}</span>
    {:else if status === 'incomplete'}
      <button class="btn primary play" data-autofocus onclick={() => (app.page = 'setup')}>
        <Icon name="arrow" size={20} />
        {tr('Continue the setup')}
      </button>
      <span class="hint faint">{tr('The build stopped before the end. Finished steps are kept: continuing picks up from there.')}</span>
    {:else}
      <button class="btn primary play" data-autofocus onclick={() => (app.page = 'setup')}>
        <Icon name="arrow" size={20} />
        {tr('Set up the game')}
      </button>
      <span class="hint faint">{tr('About 10 minutes on a fast PC. Your game file is never changed.')}</span>
    {/if}
    {#if app.game.lastExit !== null && app.game.lastExit !== 0}
      <span class="chip err">{tr('The game closed with code {code}', { code: app.game.lastExit })}</span>
    {/if}
  </div>

  <!-- What the build needs from the player, each with its action: a status block as the Display page's DLSS one. -->
  {#if built && !nupOk}
    <div class="card notice">
      <span class="chip err"><Icon name="warn" size={15} /> {tr('Your game file has moved')}</span>
      <p class="help">{tr('The game reads from your .nup while you play, and it is not where it was when the game was built. Put it back there, or set up again with the file where it is now.')}</p>
      <p class="mono where">{app.launcher?.game.nup ?? ''}</p>
      <div class="actions">
        <button class="btn" onclick={() => (app.page = 'setup')}>{tr('Choose game file')} <Icon name="arrow" size={16} /></button>
      </div>
    </div>
  {:else if engineChanged}
    <div class="card notice">
      <span class="chip warn"><Icon name="refresh" size={15} /> {tr('Rebuild available')}</span>
      <p class="help">{tr('The engine was updated since your game was built. Rebuild to get its fixes: your game files, settings and saves are kept. The game you have still plays.')}</p>
      <div class="actions">
        <button class="btn" disabled={app.game.running} onclick={() => (app.page = 'setup')}><Icon name="refresh" size={16} /> {tr('Rebuild the game')}</button>
      </div>
    </div>
  {/if}

  <div class="grid">
    <div class="card">
      <h3>{tr('Quick settings')}</h3>
      {#each quick as s (s.key)}<SettingRow {s} />{/each}
      <button class="btn ghost more" onclick={() => (app.page = 'display')}>{tr('All display settings')} <Icon name="arrow" size={16} /></button>
    </div>
    <div class="card">
      <h3>{tr('Controllers')}</h3>
      {#if app.pads.length}
        <ul class="pads">
          {#each app.pads as p (p.id)}
            <li>
              <span class="pname">{p.name}</span>
              {#if p.battery !== null}<span class="faint">{p.battery}%</span>{/if}
            </li>
          {/each}
        </ul>
      {:else}
        <p class="muted">{tr('No controller connected. Xbox, PlayStation, Switch and Switch 2 controllers work natively: just connect one.')}</p>
      {/if}
      <button class="btn ghost more" onclick={() => (app.page = 'controls')}>{tr('Test and assign controllers')} <Icon name="arrow" size={16} /></button>
    </div>
  </div>
</section>

<style>
  .home {
    display: flex;
    flex-direction: column;
    gap: 26px;
  }
  .hero {
    display: flex;
    align-items: center;
    gap: 28px;
    padding: 28px 30px;
    border-radius: var(--radius-l);
    border: 1px solid var(--line);
    background:
      radial-gradient(ellipse at 15% 50%, rgba(63, 198, 255, 0.13), transparent 55%),
      radial-gradient(ellipse at 90% 0%, rgba(231, 183, 90, 0.12), transparent 50%),
      repeating-radial-gradient(circle at 15% 50%, rgba(255, 255, 255, 0.025) 0 2px, transparent 2px 14px),
      var(--surface-2);
  }
  .emblem {
    filter: drop-shadow(0 8px 24px rgba(63, 198, 255, 0.25));
  }
  h1 {
    font-size: 40px;
    letter-spacing: 0.06em;
    background: linear-gradient(180deg, #fff6df, var(--gold));
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
  .sub {
    margin-top: 6px;
    color: var(--text-dim);
    font-size: 16px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 16px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 11px;
    border-radius: 99px;
    background: var(--surface-3);
    border: 1px solid var(--line);
    font-size: 13px;
    color: var(--text-dim);
  }
  .chip.ok {
    color: var(--ok);
    background: var(--ok-soft);
    border-color: transparent;
  }
  .chip.warn {
    color: var(--warn);
    background: var(--warn-soft);
    border-color: transparent;
  }
  .chip.err {
    color: var(--err);
    background: var(--err-soft);
    border-color: transparent;
  }
  .play-row {
    display: flex;
    align-items: center;
    gap: 18px;
    flex-wrap: wrap;
  }
  .play {
    min-height: 58px;
    min-width: 240px;
    font-size: 19px;
    font-family: var(--font-display);
    letter-spacing: 0.1em;
    border-radius: 14px;
    box-shadow: 0 10px 30px rgba(231, 183, 90, 0.22);
  }
  .hint {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 13.5px;
  }
  /* A card laid out as the Display page's DLSS block: chip, help text, actions. */
  .notice {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
  }
  .help {
    color: var(--text-dim);
    font-size: 13.5px;
    max-width: 62ch;
  }
  .where {
    color: var(--text-dim);
    overflow-wrap: anywhere;
    user-select: text;
  }
  .actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .grid {
    display: grid;
    grid-template-columns: 1.4fr 1fr;
    gap: 18px;
  }
  .card h3 {
    margin-bottom: 4px;
  }
  .more {
    margin-top: 12px;
  }
  .pads {
    list-style: none;
    margin: 10px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .pads li {
    display: flex;
    justify-content: space-between;
    padding: 10px 12px;
    border-radius: var(--radius-m);
    background: var(--surface-3);
  }
  .pname {
    font-weight: 600;
  }
  @media (max-width: 1180px) {
    .grid {
      grid-template-columns: 1fr;
    }
  }
</style>
