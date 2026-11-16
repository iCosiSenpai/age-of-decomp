<script lang="ts">
  let {
    value,
    min,
    max,
    step,
    format = '%g',
    unit = '',
    onchange,
    disabled = false,
    label,
  }: {
    value: number;
    min: number;
    max: number;
    step: number;
    format?: string;
    unit?: string;
    onchange: (v: number) => void;
    disabled?: boolean;
    label: string;
  } = $props();

  let local = $state(0);
  $effect(() => {
    local = value;
  });
  let decimals = $derived(/%\.(\d)f/.exec(format)?.[1] ? Number(/%\.(\d)f/.exec(format)![1]) : 0);
  let shown = $derived(local.toFixed(decimals) + (unit ? ` ${unit}` : ''));
  let pct = $derived(((local - min) / (max - min)) * 100);
</script>

<div class="slider" class:disabled>
  <input
    type="range"
    aria-label={label}
    {min}
    {max}
    {step}
    {disabled}
    bind:value={local}
    style:--pct="{pct}%"
    onchange={() => onchange(local)}
  />
  <output>{shown}</output>
</div>

<style>
  .slider {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 280px;
  }
  .disabled {
    opacity: 0.45;
  }
  input {
    -webkit-appearance: none;
    appearance: none;
    flex: 1;
    height: 6px;
    border-radius: 3px;
    background: linear-gradient(90deg, var(--focus) var(--pct), var(--surface-4) var(--pct));
    cursor: pointer;
  }
  input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--text);
    border: 3px solid var(--focus);
  }
  input:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 6px;
  }
  output {
    min-width: 64px;
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--text);
    font-weight: 600;
  }
</style>
