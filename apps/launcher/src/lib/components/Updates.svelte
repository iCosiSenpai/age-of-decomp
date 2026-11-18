<script lang="ts">
  import { app } from '../state.svelte';
  import { tr } from '../i18n.svelte';
  import Icon from './Icon.svelte';
  let { startup = false }: { startup?: boolean } = $props();
  let busy = $derived(app.prepare.running || app.game.running);
</script>

{#if !startup || app.update.info?.available || app.update.installing}
  <section class:banner={startup} class="updates" aria-label={tr('Compiler updates')}>
    <div>
      {#if app.update.installing}
        <strong>{tr('Downloading update… {percent}%', { percent: Math.round(app.update.progress * 100) })}</strong>
        <p>{tr('The compiler will restart when the update is ready.')}</p>
      {:else if app.update.info?.available}
        <strong>{tr('Age of Decomp {version} is available', { version: app.update.info.latest ?? '' })}</strong>
        <p>{tr('Update the compiler and engine bundle. Your game, settings and saves are kept.')}</p>
      {:else}
        <strong>{tr('Compiler updates')}</strong>
        <p>{app.update.checking ? tr('Checking for updates…') : app.update.info?.latest ? tr('The compiler is up to date.') : app.update.info ? tr('No release is available yet.') : tr('Check GitHub for a new compiler version.')}</p>
      {/if}
      {#if app.update.error}<p class="error" role="alert">{tr(app.update.error)}</p>{/if}
      {#if busy && app.update.info?.available}<p>{tr('Close the game and wait for setup to finish before updating.')}</p>{/if}
    </div>
    <div class="actions">
      {#if app.update.info?.available}
        <button class="btn primary" disabled={busy || app.update.installing} onclick={() => app.installUpdate()}>
          <Icon name="refresh" size={16} /> {tr('Update and restart')}
        </button>
      {/if}
      {#if !startup}
        <button class="btn" disabled={app.update.checking || app.update.installing} onclick={() => app.checkUpdates()}>
          <Icon name="refresh" size={16} /> {tr('Check for updates')}
        </button>
      {/if}
    </div>
  </section>
{/if}

<style>
  .updates { display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 16px; }
  .banner { padding: 14px 20px; border-bottom: 1px solid var(--line); background: var(--surface-2); }
  p { color: var(--text-dim); margin: 5px 0 0; max-width: 680px; overflow-wrap: anywhere; }
  .error { color: var(--err); }
  .actions { display: flex; gap: 10px; flex-wrap: wrap; }
</style>
