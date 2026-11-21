<script lang="ts">
  // One setting from catalog.json: label, help, restart badge and the matching control.
  import { tr, trc } from '../i18n.svelte';
  import { app } from '../state.svelte';
  import { padOfValue, padTitle, padValue } from '../controls';
  import type { CatalogSetting } from '../types';
  import Segmented from './Segmented.svelte';
  import Cycle from './Cycle.svelte';
  import Toggle from './Toggle.svelte';
  import Slider from './Slider.svelte';

  let { s }: { s: CatalogSetting } = $props();

  let value = $derived(app.value(s));
  let off = $derived(app.inactive(s));
  let changed = $derived(app.settings[s.key] !== undefined && app.settings[s.key] !== s.default);

  let dynamicOptions = $derived.by(() => {
    if (s.source === 'gpus') {
      return [
        { value: '', label: tr('Automatic') },
        ...(app.system?.gpus ?? []).map((g) => ({ value: String(g.index), label: g.name })),
      ];
    }
    if (s.source === 'pads') {
      const opts = [
        { value: 'auto', label: tr('Automatic') },
        { value: 'keyboard', label: tr('Keyboard and mouse') },
        { value: 'none', label: tr('pads|None') },
      ];
      // pad:<SDL GUID>:<k>, k counting the connected pads of that GUID from 0 (runtime/input.c pad_value)
      for (const p of app.pads) opts.push({ value: padValue(app.pads, p), label: padTitle(app.pads, p) });
      if (!opts.some((o) => o.value === value)) {
        // A value an older launcher wrote names its pad another way: the game still finds that pad, so it is shown.
        const known = padOfValue(app.pads, value);
        opts.push({ value, label: known ? padTitle(app.pads, known) : tr('A controller not connected now') });
      }
      return opts;
    }
    return [];
  });

  let choiceOptions = $derived((s.choices ?? []).map((c) => ({ value: c.value, label: trc(c.label) })));
</script>

<div class="row" class:off>
  <div class="text">
    <div class="label">
      {trc(s.label)}
      {#if s.restart}<span class="badge" title={tr('Applies the next time the game starts')}>{tr('restart')}</span>{/if}
      {#if changed}<span class="changed" title={tr('Changed from the default')}></span>{/if}
    </div>
    <p class="help">{trc(s.help)}</p>
  </div>
  <div class="control">
    {#if s.kind === 'toggle'}
      <Toggle label={trc(s.label)} checked={value === '1'} disabled={off} onchange={(v) => app.set(s.key, v ? '1' : '0')} />
    {:else if s.kind === 'slider'}
      <Slider
        label={trc(s.label)}
        value={Number(value)}
        min={s.min ?? 0}
        max={s.max ?? 100}
        step={s.step ?? 1}
        format={s.format}
        unit={s.unit}
        disabled={off}
        onchange={(v) => app.set(s.key, String(Number(v.toFixed(4))))}
      />
    {:else if s.kind === 'dynamic'}
      <Cycle label={trc(s.label)} options={dynamicOptions} {value} disabled={off} onchange={(v) => app.set(s.key, v)} />
    {:else if choiceOptions.length <= 4}
      <Segmented label={trc(s.label)} options={choiceOptions} {value} disabled={off} onchange={(v) => app.set(s.key, v)} />
    {:else}
      <Cycle label={trc(s.label)} options={choiceOptions} {value} disabled={off} onchange={(v) => app.set(s.key, v)} />
    {/if}
  </div>
</div>

<style>
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px 24px;
    padding: 16px 4px;
    border-bottom: 1px solid var(--line);
  }
  .text {
    flex: 1 1 300px;
    min-width: 0;
  }
  .control {
    flex: 0 1 auto;
    margin-left: auto;
  }
  .row:last-child {
    border-bottom: 0;
  }
  .row.off .text {
    opacity: 0.5;
  }
  .label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 620;
    font-size: 15.5px;
  }
  .help {
    margin-top: 3px;
    color: var(--text-dim);
    font-size: 13.5px;
    max-width: 62ch;
  }
  .badge {
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 2px 7px;
    border-radius: 99px;
    color: var(--warn);
    background: var(--warn-soft);
    font-weight: 700;
  }
  .changed {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--focus);
  }
  .control {
    display: flex;
    justify-content: flex-end;
  }
</style>
