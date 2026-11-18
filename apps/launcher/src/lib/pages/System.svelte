<script lang="ts">
  import { api } from '../api';
  import { resolveLocale, setLocale, tr } from '../i18n.svelte';
  import { app } from '../state.svelte';
  import Segmented from '../components/Segmented.svelte';
  import Toggle from '../components/Toggle.svelte';
  import Icon from '../components/Icon.svelte';
  import Updates from '../components/Updates.svelte';
  import type { UiLanguage } from '../types';

  let licenses = $state<string | null>(null);

  async function setLanguage(v: string) {
    app.launcher = await api.setUi({ language: v as UiLanguage });
    setLocale(resolveLocale(app.launcher.ui.language, app.launcher.system_locale));
  }
</script>

<section class="page">
  <h2>{tr('System')}</h2>

  <div class="card">
    <Updates />
    <div class="row">
      <div class="l">{tr('Check for updates at startup')}</div>
      <Toggle label={tr('Check for updates at startup')} checked={app.launcher?.ui.check_updates ?? true}
        onchange={async (v) => (app.launcher = await api.setUi({ check_updates: v }))} />
    </div>
  </div>

  <div class="card">
    <h3>{tr('Launcher')}</h3>
    <div class="row">
      <div><div class="l">{tr('Language')}</div><div class="faint">{tr('Automatic follows your system.')}</div></div>
      <Segmented
        label={tr('Language')}
        options={[{ value: 'auto', label: tr('Automatic') }, { value: 'en', label: 'English' }, { value: 'it', label: 'Italiano' }]}
        value={app.launcher?.ui.language ?? 'auto'}
        onchange={setLanguage}
      />
    </div>
    <div class="row">
      <div><div class="l">{tr('Close the launcher when the game starts')}</div><div class="faint">{tr('Otherwise it waits and shows how the session went.')}</div></div>
      <Toggle label={tr('Close the launcher when the game starts')} checked={app.launcher?.ui.close_on_play ?? false} onchange={async (v) => (app.launcher = await api.setUi({ close_on_play: v }))} />
    </div>
  </div>

  <div class="card">
    <h3>{tr('Folders')}</h3>
    <div class="folders">
      <button class="btn" onclick={() => api.openFolder('game')}><Icon name="folder" size={16} /> {tr('Game')}</button>
      <button class="btn" onclick={() => api.openFolder('saves')}><Icon name="folder" size={16} /> {tr('Saves')}</button>
      <button class="btn" onclick={() => api.openFolder('logs')}><Icon name="folder" size={16} /> {tr('Logs')}</button>
      <button class="btn" onclick={() => api.openFolder('data')}><Icon name="folder" size={16} /> {tr('Launcher data')}</button>
    </div>
    <dl class="facts">
      <dt>{tr('Game file')}</dt><dd class="mono">{app.launcher?.game.nup ?? '—'}</dd>
      <dt>{tr('Game folder')}</dt><dd class="mono">{app.launcher?.game.output ?? '—'}</dd>
      <dt>{tr('Engine')}</dt><dd class="mono">{app.launcher?.engine_dir ?? tr('not found')}</dd>
    </dl>
    <button class="btn ghost" onclick={() => (app.page = 'setup')}><Icon name="refresh" size={16} /> {tr('Check or rebuild the game')}</button>
  </div>

  <div class="card">
    <h3>{tr('About')}</h3>
    <p class="muted">
      {tr('Age of Decomp {v} is free software (GPL-3.0-or-later). Its engine is HWAoI Recomp by Alano6678 and contributors, built on DKB Recomp; shaders are translated with code from Ryujinx and Eglantine. It contains nothing from Nintendo or Koei Tecmo and is not affiliated with them.', { v: app.launcher?.version ?? '' })}
    </p>
    <button class="btn ghost" onclick={async () => (licenses = licenses ? null : await api.licenses())}>{licenses ? tr('Hide licences') : tr('Licences and credits')}</button>
    {#if licenses}<pre class="lic mono">{licenses}</pre>{/if}
  </div>
</section>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 1040px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 20px;
    padding: 8px 0;
    border-bottom: 1px solid var(--line);
  }
  .row:last-child {
    border-bottom: 0;
  }
  .l {
    font-weight: 620;
  }
  .folders {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .facts {
    display: grid;
    grid-template-columns: 140px 1fr;
    gap: 6px 14px;
    margin: 4px 0;
  }
  dt {
    color: var(--text-faint);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
    user-select: text;
  }
  .lic {
    max-height: 320px;
    overflow: auto;
    white-space: pre-wrap;
    background: #07080b;
    padding: 12px;
    border-radius: var(--radius-m);
    user-select: text;
  }
</style>
