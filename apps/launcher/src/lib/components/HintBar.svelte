<script lang="ts">
  import { tr } from '../i18n.svelte';
  import { app } from '../state.svelte';
  import Glyph from './Glyph.svelte';

  let pad = $derived(app.pads[0]);
</script>

<footer class="hints">
  <div class="left">
    {#if pad}
      <span class="pad"><span class="dot"></span>{pad.name}{#if pad.battery !== null} · {pad.battery}%{/if}</span>
      {#if app.pads.length > 1}<span class="faint">+{app.pads.length - 1}</span>{/if}
    {:else}
      <span class="faint">{tr('No controller connected: keyboard and mouse work everywhere')}</span>
    {/if}
  </div>
  <div class="right">
    <span><Glyph button="dpad" family={app.hintPad} /> {tr('Move')}</span>
    <span><Glyph button="a" family={app.hintPad} /> {tr('Select')}</span>
    <span><Glyph button="b" family={app.hintPad} /> {tr('Back')}</span>
    <span><Glyph button="start" family={app.hintPad} /> {tr('Play')}</span>
  </div>
</footer>

<style>
  .hints {
    height: var(--hintbar-h);
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 18px;
    border-top: 1px solid var(--line);
    background: var(--surface-1);
    font-size: 13px;
    color: var(--text-dim);
  }
  .left,
  .right {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .right span {
    display: inline-flex;
    align-items: center;
    gap: 7px;
  }
  .pad {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--text);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 8px var(--ok);
  }
</style>
