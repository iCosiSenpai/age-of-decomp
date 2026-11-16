<script lang="ts">
  // A settings page for one catalogue group.
  import { api } from '../api';
  import { tr, trc } from '../i18n.svelte';
  import { app, catalog } from '../state.svelte';
  import SettingRow from '../components/SettingRow.svelte';
  import Icon from '../components/Icon.svelte';

  let { group }: { group: string } = $props();

  const INTRO: Record<string, string> = {
    display: 'Resolution, window, frame rate and image quality. Changes are saved at once; the ones marked restart apply the next time the game starts.',
    picture: 'Colour adjustments applied when the picture is shown. They cost nothing in performance.',
    game: 'Language and console mode.',
    audio: 'Sound output.',
    controls: 'Players, controllers and feel. Keyboard and button bindings are in the game menu (Esc → Controls).',
    performance: 'Engine options for speed and memory. The defaults are the tested ones: change them only to compare or to work around a problem.',
    diagnostics: 'Options for bug reports. They slow the game down.',
  };

  let meta = $derived(catalog.groups.find((g) => g.id === group));
  let rows = $derived(catalog.settings.filter((s) => s.group === group && (!s.advanced || app.showAdvanced)));
  let hidden = $derived(catalog.settings.filter((s) => s.group === group && s.advanced && !app.showAdvanced).length);

  async function reset() {
    app.settings = await api.resetSettings(catalog.settings.filter((s) => s.group === group).map((s) => s.key));
    app.toast(tr('Defaults restored'), 'ok');
  }
</script>

<section class="page">
  <header>
    <div>
      <h2>{trc(meta?.label ?? group)}</h2>
      {#if INTRO[group]}<p class="muted">{trc(INTRO[group])}</p>{/if}
    </div>
    <button class="btn ghost" onclick={reset}><Icon name="refresh" size={16} /> {tr('Restore defaults')}</button>
  </header>

  {#if app.launcher?.game.status !== 'complete'}
    <div class="note"><Icon name="info" size={18} /> {tr('These settings are saved now and used once the game is built.')}</div>
  {/if}

  <div class="card list">
    {#each rows as s (s.key)}<SettingRow {s} />{/each}
  </div>
  {#if hidden}
    <button class="btn ghost more" onclick={() => (app.showAdvanced = true)}>{tr('Show {n} advanced options', { n: hidden })}</button>
  {/if}
</section>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 1040px;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 20px;
  }
  header p {
    margin-top: 6px;
    max-width: 75ch;
  }
  .list {
    padding: 4px 22px;
  }
  .note {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    border-radius: var(--radius-m);
    background: var(--focus-soft);
    color: var(--text);
  }
  .more {
    align-self: flex-start;
  }
</style>
