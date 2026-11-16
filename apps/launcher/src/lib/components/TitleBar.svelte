<script lang="ts">
  import { api } from '../api';
  import { tr } from '../i18n.svelte';
  import Icon from './Icon.svelte';
  import Logo from './Logo.svelte';
  let { version }: { version: string } = $props();
</script>

<header class="titlebar" data-tauri-drag-region>
  <div class="brand" data-tauri-drag-region>
    <Logo size={22} />
    <span class="name" data-tauri-drag-region>Age of Decomp</span>
    <span class="ver faint" data-tauri-drag-region>v{version}</span>
  </div>
  <div class="controls" data-nav-skip>
    <button class="wbtn" tabindex="-1" aria-label={tr('Minimize')} onclick={() => api.windowAction('minimize')}><Icon name="min" size={16} /></button>
    <button class="wbtn" tabindex="-1" aria-label={tr('Maximize')} onclick={() => api.windowAction('maximize')}><Icon name="max" size={14} /></button>
    <button class="wbtn close" tabindex="-1" aria-label={tr('Close')} onclick={() => api.windowAction('close')}><Icon name="close" size={16} /></button>
  </div>
</header>

<style>
  .titlebar {
    height: var(--titlebar-h);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-left: 14px;
    background: var(--surface-1);
    border-bottom: 1px solid var(--line);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .name {
    font-family: var(--font-display);
    font-weight: 700;
    letter-spacing: 0.08em;
    font-size: 14px;
  }
  .ver {
    font-size: 12px;
  }
  .controls {
    display: flex;
    height: 100%;
  }
  .wbtn {
    width: 46px;
    height: 100%;
    border: 0;
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
    display: grid;
    place-items: center;
  }
  .wbtn:hover {
    background: var(--surface-3);
    color: var(--text);
  }
  .wbtn.close:hover {
    background: #c42b1c;
    color: #fff;
  }
</style>
