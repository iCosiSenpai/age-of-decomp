<script lang="ts">
  // A row of choices; left/right on a controller moves through them (data-horizontal).
  let {
    options,
    value,
    onchange,
    disabled = false,
    label,
  }: {
    options: { value: string; label: string }[];
    value: string;
    onchange: (v: string) => void;
    disabled?: boolean;
    label: string;
  } = $props();

  function key(e: KeyboardEvent) {
    const i = options.findIndex((o) => o.value === value);
    if (e.key === 'ArrowLeft' && i > 0) {
      onchange(options[i - 1].value);
      e.preventDefault();
    } else if (e.key === 'ArrowRight' && i < options.length - 1) {
      onchange(options[i + 1].value);
      e.preventDefault();
    }
  }
</script>

<div class="seg" role="radiogroup" aria-label={label} data-horizontal tabindex={disabled ? -1 : 0} onkeydown={key} class:disabled>
  {#each options as o (o.value)}
    <button
      type="button"
      role="radio"
      tabindex="-1"
      aria-checked={o.value === value}
      class:on={o.value === value}
      {disabled}
      onclick={() => onchange(o.value)}>{o.label}</button
    >
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 4px;
    border-radius: var(--radius-m);
    background: var(--surface-1);
    border: 1px solid var(--line);
  }
  .seg.disabled {
    opacity: 0.45;
  }
  button {
    border: 0;
    border-radius: 7px;
    padding: 7px 13px;
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
    font-size: 13.5px;
    font-weight: 550;
    white-space: nowrap;
  }
  button:hover:not(:disabled) {
    color: var(--text);
    background: var(--surface-3);
  }
  button.on {
    background: var(--surface-4);
    color: var(--text);
    box-shadow: inset 0 0 0 1px var(--focus);
  }
</style>
