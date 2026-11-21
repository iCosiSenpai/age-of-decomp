<script lang="ts">
  // A settings page for one catalogue group. The Controls group has its own page (Pads.svelte).
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

  // NVIDIA DLSS (Upscaling = NVIDIA DLSS): NVIDIA's own nvngx_dlss.dll, which Age of Decomp cannot ship (src-tauri/src/dlss.rs). Its
  // status, the download and its error live on `app.dlss`, so leaving this page mid-download keeps them; only the
  // licence question is this page's own, and leaving the page closes it unanswered.
  let dlssAsking = $state(false);
  let wantsDlss = $derived(group === 'display' && app.settings.temporal === 'dlss');
  $effect(() => {
    if (wantsDlss) void app.loadDlss();
  });

  function dlssDownload() {
    dlssAsking = false;
    void app.downloadDlss();
  }

  async function dlssChoose() {
    const file = await api.pickDlssFile(tr("Choose NVIDIA's nvngx_dlss.dll"));
    if (file) void app.pickDlss(file);
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
    {#each rows as s (s.key)}
      <SettingRow {s} />
      {#if s.key === 'temporal' && wantsDlss && app.dlss.status}
        <div class="dlss">
          {#if !app.dlss.status.rtx}
            <span class="chip warn"><Icon name="warn" size={15} /> {tr('Needs an NVIDIA RTX card')}</span>
            <p class="help">{tr('DLSS runs only on NVIDIA RTX cards. On this PC the game plays without it.')}</p>
          {:else if app.dlss.status.ready}
            <span class="chip ok"><Icon name="check" size={15} /> {tr("NVIDIA's DLSS file is ready")}</span>
          {:else}
            <span class="chip warn"><Icon name="warn" size={15} /> {tr("NVIDIA's DLSS file is missing")}</span>
            {#if app.dlss.downloading}
              <strong>{tr("Downloading NVIDIA's DLSS file… {percent}%", { percent: Math.round(app.dlss.progress * 100) })}</strong>
            {:else if app.dlss.checking}
              <strong>{tr('Checking the file…')}</strong>
            {/if}
            {#if dlssAsking}
              <!-- A consent, written as the family's other one (App.svelte): two plain buttons, the refusal first. -->
              <p class="help">{tr("NVIDIA DLSS is NVIDIA's software, not part of Age of Decomp. Downloading it from NVIDIA's GitHub means you accept NVIDIA's DLSS licence.")}</p>
              <div class="actions">
                <button class="btn ghost" onclick={() => api.dlssLicense()}>{tr("NVIDIA's licence")}</button>
                <button class="btn" data-autofocus onclick={() => (dlssAsking = false)}>{tr('Cancel')}</button>
                <button class="btn" onclick={dlssDownload}>{tr('Accept and download')}</button>
              </div>
            {:else}
              <p class="help">{tr("DLSS needs NVIDIA's own DLSS file, which Age of Decomp cannot include. Download it from NVIDIA, or choose a copy you already have.")}</p>
              <div class="actions">
                <button class="btn" disabled={app.dlss.downloading || app.dlss.checking} onclick={() => (dlssAsking = true)}>{tr('Download from NVIDIA…')}</button>
                <button class="btn" disabled={app.dlss.downloading || app.dlss.checking} onclick={dlssChoose}>{tr('Choose the file…')}</button>
              </div>
            {/if}
          {/if}
          {#if app.dlss.error}<p class="error" role="alert">{tr(app.dlss.error)}</p>{/if}
        </div>
      {/if}
    {/each}
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
  /* The DLSS file block under the Anti-aliasing row: a row's inset and divider (SettingRow .row), chips as Home's,
     text as a row's help, actions as Updates'. Top 12px: the row above keeps its own divider. */
  .dlss {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
    padding: 12px 4px 16px;
    border-bottom: 1px solid var(--line);
  }
  .dlss:last-child {
    border-bottom: 0;
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
  .help {
    color: var(--text-dim);
    font-size: 13.5px;
    max-width: 62ch;
  }
  .actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .error {
    color: var(--err);
    max-width: 680px;
    overflow-wrap: anywhere;
  }
</style>
