<script lang="ts">
  // ‹ value › picker for long lists (languages, resolutions, devices); left/right step through it.
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

  let index = $derived(Math.max(0, options.findIndex((o) => o.value === value)));
  let current = $derived(options.find((o) => o.value === value)?.label ?? value);

  function step(d: number) {
    if (disabled || !options.length) return;
    const i = (index + d + options.length) % options.length;
    onchange(options[i].value);
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowLeft') {
      step(-1);
      e.preventDefault();
    } else if (e.key === 'ArrowRight') {
      step(1);
      e.preventDefault();
    }
  }
</script>

<div class="cycle" class:disabled role="spinbutton" aria-label={label} aria-valuetext={current} tabindex={disabled ? -1 : 0} data-horizontal onkeydown={key}>
  <button type="button" tabindex="-1" aria-label="‹" onclick={() => step(-1)} {disabled}>‹</button>
  <span class="value">{current}</span>
  <button type="button" tabindex="-1" aria-label="›" onclick={() => step(1)} {disabled}>›</button>
</div>

<style>
  .cycle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 280px;
    padding: 4px;
    border-radius: var(--radius-m);
    background: var(--surface-1);
    border: 1px solid var(--line);
  }
  .disabled {
    opacity: 0.45;
  }
  .value {
    flex: 1;
    text-align: center;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  button {
    width: 32px;
    height: 32px;
    border: 0;
    border-radius: 7px;
    background: var(--surface-3);
    cursor: pointer;
    font-size: 18px;
    line-height: 1;
  }
  button:hover:not(:disabled) {
    background: var(--surface-4);
  }
</style>
