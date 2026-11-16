<script lang="ts">
  import { tr } from '../i18n.svelte';
  import { app, NAV } from '../state.svelte';
  import Icon from './Icon.svelte';
  import Glyph from './Glyph.svelte';

  let items = $derived(NAV.filter((n) => !n.advanced || app.showAdvanced));
  let built = $derived(app.launcher?.game.status === 'complete');
</script>

<nav class="rail" aria-label={tr('Pages')}>
  <div class="tabs-hint faint">
    <Glyph button="lb" family={app.hintPad} />
    <span>{tr('Pages')}</span>
    <Glyph button="rb" family={app.hintPad} />
  </div>
  {#each items as item (item.id)}
    <button
      class="item"
      class:active={app.page === item.id}
      aria-current={app.page === item.id ? 'page' : undefined}
      onclick={() => (app.page = item.id)}
    >
      <span class="icon"><Icon name={item.icon} /></span>
      <span class="label">{tr(item.label)}</span>
      {#if item.id === 'setup' && !built}
        <span class="dot warn" aria-hidden="true"></span>
      {:else if item.id === 'home' && app.game.running}
        <span class="dot ok" aria-hidden="true"></span>
      {/if}
    </button>
  {/each}
  <div class="spacer"></div>
  <label class="adv">
    <input type="checkbox" bind:checked={app.showAdvanced} />
    <span>{tr('Advanced pages')}</span>
  </label>
</nav>

<style>
  .rail {
    width: var(--rail-w);
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 14px 12px;
    background: linear-gradient(180deg, var(--surface-1), #0d0f13);
    border-right: 1px solid var(--line);
    overflow-y: auto;
  }
  .tabs-hint {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 12px;
    padding: 0 6px 12px;
  }
  .item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 10px 12px;
    border: 1px solid transparent;
    border-radius: var(--radius-m);
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
    text-align: left;
    font-weight: 550;
    transition: background 120ms var(--ease), color 120ms var(--ease);
  }
  .item:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .item.active {
    background: var(--focus-soft);
    border-color: rgba(63, 198, 255, 0.35);
    color: var(--text);
  }
  .item.active .icon {
    color: var(--focus);
  }
  .icon {
    display: grid;
    place-items: center;
  }
  .dot {
    position: absolute;
    right: 12px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }
  .dot.warn {
    background: var(--warn);
  }
  .dot.ok {
    background: var(--ok);
  }
  .spacer {
    flex: 1;
  }
  .adv {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-faint);
    padding: 8px 10px;
    cursor: pointer;
  }
  .adv input {
    accent-color: var(--focus);
  }
</style>
