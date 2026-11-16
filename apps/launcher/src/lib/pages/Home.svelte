<script lang="ts">
  import { api } from '../api';
  import { tr } from '../i18n.svelte';
  import { app, catalog } from '../state.svelte';
  import Logo from '../components/Logo.svelte';
  import Icon from '../components/Icon.svelte';
  import Glyph from '../components/Glyph.svelte';
  import SettingRow from '../components/SettingRow.svelte';

  let built = $derived(app.launcher?.game.status === 'complete');
  let nupOk = $derived(app.launcher?.game.nup_present ?? false);
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
        {#if built}
          <span class="chip ok"><Icon name="check" size={15} /> {tr('Ready to play')}</span>
        {:else}
          <span class="chip warn"><Icon name="warn" size={15} /> {tr('Setup needed')}</span>
        {/if}
        {#if built && !nupOk}
          <span class="chip err"><Icon name="warn" size={15} /> {tr('Your game file has moved')}</span>
        {/if}
        <span class="chip"><Icon name="display" size={15} /> {app.settings.output_res ?? '1920x1080'} · {app.settings.fps_limit ?? '60'} FPS</span>
        {#if app.system?.gpus[0]}<span class="chip"><Icon name="chip" size={15} /> {app.system.gpus[0].name}</span>{/if}
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
      <button class="btn ghost more" onclick={() => (app.page = 'pads')}>{tr('Test and assign controllers')} <Icon name="arrow" size={16} /></button>
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
