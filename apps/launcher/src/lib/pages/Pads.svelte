<script lang="ts">
  // Connected controllers (through SDL 3, the same input layer as the game): type, features, battery, a live tester,
  // rumble test and player assignment.
  import { onDestroy } from 'svelte';
  import { api } from '../api';
  import { tr } from '../i18n.svelte';
  import { app } from '../state.svelte';
  import Icon from '../components/Icon.svelte';
  import type { Pad, PadState } from '../types';

  let selected = $state<number | null>(null);
  let live = $state<PadState | null>(null);
  let timer: ReturnType<typeof setInterval> | null = null;

  let pad = $derived(app.pads.find((p) => p.id === selected) ?? app.pads[0] ?? null);

  $effect(() => {
    const id = pad?.id;
    if (timer) clearInterval(timer);
    live = null;
    if (id === undefined) return;
    timer = setInterval(async () => {
      live = await api.padState(id).catch(() => null);
    }, 50);
  });
  onDestroy(() => timer && clearInterval(timer));

  function family(p: Pad): string {
    if (p.type.startsWith('ps')) return 'PlayStation';
    if (p.type.startsWith('xbox')) return 'Xbox';
    if (p.type === 'gamecube') return 'GameCube';
    if (p.type.startsWith('switch') || p.type.startsWith('joycon')) return p.gen2 ? 'Nintendo Switch 2' : 'Nintendo Switch';
    return tr('Generic');
  }

  function slot(p: Pad): string {
    const same = app.pads.filter((q) => q.guid === p.guid);
    return `pad:${p.guid}:${same.indexOf(p) + 1}`;
  }

  async function assign(player: 1 | 2, p: Pad) {
    await app.set(`player${player}_device`, slot(p));
    app.toast(tr('{name} is now player {n}', { name: p.name, n: player }), 'ok');
  }

  const BUTTONS: [string, string][] = [
    ['south', 'A / ✕'], ['east', 'B / ○'], ['west', 'X / □'], ['north', 'Y / △'],
    ['left_shoulder', 'L'], ['right_shoulder', 'R'], ['back', '−'], ['start', '+'],
    ['left_stick', 'L3'], ['right_stick', 'R3'], ['guide', '⌂'], ['misc1', '◉'],
    ['dpad_up', '↑'], ['dpad_down', '↓'], ['dpad_left', '←'], ['dpad_right', '→'],
  ];
</script>

<section class="page">
  <header>
    <h2>{tr('Controllers')}</h2>
    <p class="muted">{tr('Xbox, DualSense, DualShock 4, Switch Pro, Joy-Con, Switch 2 Pro Controller, Joy-Con 2 and GameCube controllers work natively, wired or Bluetooth, with rumble and motion where the controller has them.')}</p>
  </header>

  {#if !app.pads.length}
    <div class="card empty">
      <Icon name="controls" size={40} />
      <p>{tr('No controller connected. Connect one with a cable or pair it in your system’s Bluetooth settings: it shows up here at once.')}</p>
    </div>
  {:else}
    <div class="layout">
      <div class="list">
        {#each app.pads as p (p.id)}
          <button class="padcard" class:sel={pad?.id === p.id} onclick={() => (selected = p.id)}>
            <div class="top">
              <span class="pname">{p.name}</span>
              {#if p.battery !== null}<span class="bat" class:low={p.battery < 25}>{p.battery}%</span>{/if}
            </div>
            <div class="chips">
              <span class="chip">{family(p)}</span>
              {#if p.wired !== null}<span class="chip">{p.wired ? tr('Wired') : tr('Wireless')}</span>{/if}
              {#if p.has_gyro}<span class="chip">{tr('Motion')}</span>{/if}
              {#if p.has_rumble}<span class="chip">{tr('Rumble')}</span>{/if}
              {#if p.has_trigger_rumble}<span class="chip">{tr('Trigger rumble')}</span>{/if}
              {#if p.has_led}<span class="chip">{tr('led|Light')}</span>{/if}
            </div>
          </button>
        {/each}
      </div>

      {#if pad}
        <div class="card tester">
          <div class="trow">
            <h3>{pad.name}</h3>
            <div class="assign">
              <button class="btn" onclick={() => assign(1, pad!)}>{tr('Use as player 1')}</button>
              <button class="btn" onclick={() => assign(2, pad!)}>{tr('Use as player 2')}</button>
            </div>
          </div>
          <div class="sticks">
            {#each [['leftx', 'lefty', tr('Left stick')], ['rightx', 'righty', tr('Right stick')]] as [x, y, label] (label)}
              <div class="stick">
                <div class="ring"><span class="dot" style:left="{50 + (live?.axes[x] ?? 0) * 42}%" style:top="{50 + (live?.axes[y] ?? 0) * 42}%"></span></div>
                <span class="faint">{label}</span>
              </div>
            {/each}
            <div class="triggers">
              {#each [['left_trigger', 'ZL / L2 / LT'], ['right_trigger', 'ZR / R2 / RT']] as [k, label] (k)}
                <div class="trig"><div class="tfill" style:height="{(live?.axes[k] ?? 0) * 100}%"></div></div>
                <span class="faint tl">{label}</span>
              {/each}
            </div>
          </div>
          <div class="buttons">
            {#each BUTTONS as [k, label] (k)}
              <span class="b" class:on={live?.buttons[k]}>{label}</span>
            {/each}
          </div>
          {#if pad.has_gyro}
            <div class="gyro">
              <span class="faint">{tr('Motion')}</span>
              {#each [0, 1, 2] as axis (axis)}
                <div class="gbar"><div class="gfill" style:width="{Math.min(100, Math.abs(live?.gyro?.[axis] ?? 0) * 40)}%"></div></div>
              {/each}
            </div>
          {/if}
          {#if pad.has_rumble}
            <div class="rumble">
              <span class="faint">{tr('Rumble test')}</span>
              <button class="btn" onclick={() => api.rumble(pad!.id, 0.35, 0.15, 400)}><Icon name="vibrate" size={16} /> {tr('Light')}</button>
              <button class="btn" onclick={() => api.rumble(pad!.id, 1, 1, 600)}><Icon name="vibrate" size={16} /> {tr('Strong')}</button>
            </div>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 1100px;
  }
  header p {
    margin-top: 6px;
    max-width: 80ch;
  }
  .empty {
    display: flex;
    align-items: center;
    gap: 18px;
    color: var(--text-dim);
  }
  .layout {
    display: grid;
    grid-template-columns: 340px 1fr;
    gap: 16px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .padcard {
    text-align: left;
    padding: 14px;
    border-radius: var(--radius-m);
    border: 1px solid var(--line);
    background: var(--surface-2);
    cursor: pointer;
  }
  .padcard.sel {
    border-color: var(--focus);
    background: var(--focus-soft);
  }
  .top {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }
  .pname {
    font-weight: 650;
  }
  .bat {
    color: var(--ok);
    font-weight: 600;
  }
  .bat.low {
    color: var(--warn);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 8px;
  }
  .chip {
    font-size: 12px;
    padding: 2px 8px;
    border-radius: 99px;
    background: var(--surface-4);
    color: var(--text-dim);
  }
  .tester {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .trow {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }
  .assign {
    display: flex;
    gap: 8px;
  }
  .sticks {
    display: flex;
    align-items: center;
    gap: 28px;
  }
  .stick {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }
  .ring {
    position: relative;
    width: 110px;
    height: 110px;
    border-radius: 50%;
    border: 2px solid var(--line-strong);
    background: radial-gradient(circle, var(--surface-3), var(--surface-1));
  }
  .dot {
    position: absolute;
    width: 18px;
    height: 18px;
    margin: -9px 0 0 -9px;
    border-radius: 50%;
    background: var(--focus);
    box-shadow: 0 0 12px var(--focus);
  }
  .triggers {
    display: grid;
    grid-template-columns: auto auto;
    grid-template-rows: 110px auto;
    grid-auto-flow: column;
    gap: 6px 18px;
    align-items: end;
  }
  .trig {
    width: 26px;
    height: 110px;
    border-radius: 6px;
    background: var(--surface-1);
    border: 1px solid var(--line);
    display: flex;
    align-items: flex-end;
    overflow: hidden;
  }
  .tfill {
    width: 100%;
    background: var(--gold);
  }
  .tl {
    font-size: 11px;
  }
  .buttons {
    display: grid;
    grid-template-columns: repeat(8, 1fr);
    gap: 6px;
  }
  .b {
    padding: 8px 0;
    text-align: center;
    border-radius: 8px;
    background: var(--surface-3);
    font-size: 12.5px;
    font-weight: 650;
    color: var(--text-dim);
  }
  .b.on {
    background: var(--focus);
    color: #06202c;
  }
  .gyro,
  .rumble {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .gbar {
    flex: 1;
    height: 8px;
    border-radius: 4px;
    background: var(--surface-1);
    overflow: hidden;
  }
  .gfill {
    height: 100%;
    background: var(--ok);
  }
</style>
