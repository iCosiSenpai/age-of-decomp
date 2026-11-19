<script lang="ts">
  // A controller button as the player's pad labels it: Xbox letters, PlayStation shapes, Nintendo letters (A on the
  // right), or the keyboard key.
  import { buttonImage, type PadFamily } from '../controller-glyphs';
  type Family = PadFamily;
  type Btn = 'a' | 'b' | 'x' | 'y' | 'lb' | 'rb' | 'start' | 'dpad';
  let { button, family }: { button: Btn; family: Family } = $props();

  const KEYS: Record<Btn, string> = { a: 'Enter', b: 'Esc', x: 'X', y: 'Y', lb: 'PgUp', rb: 'PgDn', start: 'F5', dpad: '↑↓' };
  const XBOX: Record<Btn, string> = { a: 'A', b: 'B', x: 'X', y: 'Y', lb: 'LB', rb: 'RB', start: '☰', dpad: '✚' };
  // Nintendo pads confirm with A (east) and go back with B (south), as on the console: the backend maps them so.
  const NINTENDO: Record<Btn, string> = { a: 'A', b: 'B', x: 'X', y: 'Y', lb: 'L', rb: 'R', start: '+', dpad: '✚' };
  const PS: Record<Btn, string> = { a: '✕', b: '○', x: '□', y: '△', lb: 'L1', rb: 'R1', start: '≡', dpad: '✚' };
  const COLORS: Record<string, string> = { A: '#6ecb63', B: '#e5534b', X: '#4d8fe8', Y: '#e8c34d' };

  let label = $derived(
    family === 'keyboard' ? KEYS[button] : family === 'xbox' ? XBOX[button] : ['playstation', 'ps4', 'ps5'].includes(family) ? PS[button] : NINTENDO[button],
  );
  let round = $derived(family !== 'keyboard' && ['a', 'b', 'x', 'y'].includes(button));
  let tint = $derived(family === 'xbox' ? COLORS[label] : undefined);
</script>

{#if family !== 'keyboard' && buttonImage(family, button)}
  <img class="button-image" src={buttonImage(family, button)} alt={label} />
{:else}
  <span class="glyph" class:round class:key={family === 'keyboard'} style:color={tint}>{label}</span>
{/if}

<style>
  .button-image { width: 28px; height: 28px; vertical-align: middle; }
  .glyph {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 24px;
    height: 24px;
    padding: 0 6px;
    border-radius: 6px;
    border: 1.5px solid var(--line-strong);
    background: var(--surface-3);
    font-size: 12px;
    font-weight: 700;
    color: var(--text);
    line-height: 1;
  }
  .round {
    border-radius: 50%;
    padding: 0;
    width: 24px;
  }
  .key {
    font-weight: 600;
    font-size: 11px;
  }
</style>
