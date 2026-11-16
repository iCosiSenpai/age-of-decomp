<script lang="ts">
  import { app } from '../state.svelte';
</script>

<div class="toasts" aria-live="polite">
  {#each app.toasts as t (t.id)}
    <div class="toast {t.tone}">{t.text}</div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 20px;
    bottom: calc(var(--hintbar-h) + 16px);
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 50;
  }
  .toast {
    min-width: 260px;
    max-width: 440px;
    padding: 12px 16px;
    border-radius: var(--radius-m);
    background: var(--surface-3);
    border-left: 4px solid var(--focus);
    box-shadow: var(--shadow);
    font-size: 14px;
  }
  .ok {
    border-color: var(--ok);
  }
  .warn {
    border-color: var(--warn);
  }
  .err {
    border-color: var(--err);
  }
</style>
