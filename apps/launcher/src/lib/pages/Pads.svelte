<script lang="ts">
  // The Controls page: every input device the game can use (the keyboard and mouse, and each connected controller,
  // read through SDL 3 like the game does), who plays with it, where the game's buttons are on it, the settings it
  // uses, and a live test. What it says about the game comes from ../controls.ts, a mirror of the engine's own rules.
  import { onDestroy } from 'svelte';
  import { api } from '../api';
  import { tr } from '../i18n.svelte';
  import { app, catalog } from '../state.svelte';
  import Icon from '../components/Icon.svelte';
  import SettingRow from '../components/SettingRow.svelte';
  import { buttonImage, padFamily } from '../controller-glyphs';
  import { actionLabel, bindings, gamePictures, keyLabel, owners, padButtonName, padKind, padLayout, padTitle, padValue, type Action, type PadButton } from '../controls';
  import type { Pad, PadState } from '../types';

  // The device the panel shows: the keyboard and mouse, or a pad by its id. It opens on the device in use; when the
  // controllers are only known after the page is up (the launcher has just started), it moves to that one once,
  // unless the player has already chosen.
  let selected = $state<number | 'keyboard'>(app.usedPad?.id ?? 'keyboard');
  let chosen = app.pads.length > 0;
  $effect(() => {
    const used = app.usedPad;
    if (chosen || !app.pads.length) return;
    chosen = true;
    if (used) selected = used.id;
  });
  function choose(device: number | 'keyboard') {
    chosen = true;
    selected = device;
  }
  let pad = $derived(typeof selected === 'number' ? (app.pads.find((p) => p.id === selected) ?? null) : null);

  let own = $derived(owners(app.pads, app.settings));
  // The player the shown device plays as in the game (0 or 1); -1 when the game does not read it.
  let player = $derived(pad ? (own.pads.get(pad.id) ?? -1) : own.keyboard);
  // The bindings shown are that player's; a device the game does not read shows player 1's.
  let binds = $derived(bindings(app.settings, Math.max(player, 0), padLayout(app.settings)));

  let live = $state<PadState | null>(null);
  let timer: ReturnType<typeof setInterval> | null = null;
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

  // The catalogue's Controls settings, each where it applies: a controller's own (only the ones it has the hardware
  // for), the keyboard and mouse's, and the rest (who plays with what) for every device. A setting that applies to
  // both a controller and the keyboard is in both lists.
  const PAD_KEYS = ['button_prompts', 'confirm_button', 'stick_deadzone', 'camera_speed', 'motion', 'rumble_strength',
    'rumble_curve', 'rumble_triggers'];
  const KEYBOARD_KEYS = ['button_prompts', 'camera_speed', 'mouse_sens'];
  const controls = catalog.settings.filter((s) => s.group === 'controls');
  function has(p: Pad, key: string): boolean {
    if (key === 'motion') return p.has_gyro && p.has_accel;
    if (key === 'rumble_strength' || key === 'rumble_curve') return p.has_rumble;
    if (key === 'rumble_triggers') return p.has_trigger_rumble;
    return true;
  }
  function ofDevice(key: string): boolean {
    return PAD_KEYS.includes(key) || KEYBOARD_KEYS.includes(key);
  }
  // Whether a device's setting is for the device shown.
  function applies(key: string): boolean {
    return pad ? PAD_KEYS.includes(key) && has(pad, key) : KEYBOARD_KEYS.includes(key);
  }
  let shown = $derived(controls.filter((s) => !s.advanced || app.showAdvanced));
  let playerRows = $derived(shown.filter((s) => !ofDevice(s.key)));
  let optionRows = $derived(shown.filter((s) => applies(s.key)));
  // The advanced settings "Show n advanced options" would add for the device shown.
  let hidden = $derived(app.showAdvanced ? 0 : controls.filter((s) => s.advanced && (!ofDevice(s.key) || applies(s.key))).length);
  // Whose buttons the game's own pictures show (button_prompts).
  let pictures = $derived(gamePictures(app.settings));
  // A back button has no game button until the player gives it one in the game's menu.
  let paddleBound = $derived([...binds.pad.values()].some((list) => list.some((b) => b.includes('paddle'))));

  function reset() {
    return app.reset(controls.map((s) => s.key), tr('Defaults restored'));
  }

  async function assign(n: 1 | 2) {
    const value = pad ? padValue(app.pads, pad) : 'keyboard';
    const other = n === 1 ? 2 : 1;
    if (!(await app.set(`player${n}_device`, value))) return;   // the reason is in the toast set() showed
    // One device, one player: the other player goes back to Automatic when it had this one.
    if (app.settings[`player${other}_device`] === value && !(await app.set(`player${other}_device`, 'auto'))) return;
    app.toast(tr('{name} is now player {n}', { name: pad ? padTitle(app.pads, pad) : tr('Keyboard and mouse'), n }), 'ok');
  }

  // The game's buttons, in the order the page lists them, and the stick directions the keys drive.
  const BUTTONS: Action[] = ['A', 'B', 'X', 'Y', 'L', 'R', 'ZL', 'ZR', 'PLUS', 'MINUS', 'LCLICK', 'RCLICK', 'UP', 'DOWN', 'LEFT', 'RIGHT'];
  const MOVE: Action[] = ['LSTICK_UP', 'LSTICK_LEFT', 'LSTICK_DOWN', 'LSTICK_RIGHT'];
  const CAMERA: Action[] = ['RSTICK_UP', 'RSTICK_LEFT', 'RSTICK_DOWN', 'RSTICK_RIGHT'];

  function keysOf(actions: Action[]): string {
    return actions.flatMap((a) => binds.keys.get(a) ?? []).map(keyLabel).join(' · ');
  }
  function buttonsOf(a: Action): PadButton[] {
    return binds.pad.get(a) ?? [];
  }
  function image(p: Pad, b: PadButton): string | undefined {
    const name = padButtonName(p, b).image;
    return name ? buttonImage(padFamily(p.type), name) : undefined;
  }
  // Two of the game's buttons held together, as this pad names them: "View and Menu together".
  function combo(p: Pad, a: Action, b: Action): string {
    const [x, y] = [buttonsOf(a)[0], buttonsOf(b)[0]];
    return x && y ? tr('{a} and {b} together', { a: padButtonName(p, x).text, b: padButtonName(p, y).text }) : '—';
  }

  // The tester's cells: the backend's button names (src-tauri/src/gamepad.rs BUTTONS) and the pad button they are.
  const TEST: [string, PadButton | 'guide' | 'misc1'][] = [
    ['south', 'south'], ['east', 'east'], ['west', 'west'], ['north', 'north'],
    ['left_shoulder', 'l'], ['right_shoulder', 'r'], ['back', 'back'], ['start', 'start'],
    ['left_stick', 'lstick'], ['right_stick', 'rstick'], ['guide', 'guide'], ['misc1', 'misc1'],
    ['dpad_up', 'dpad_up'], ['dpad_down', 'dpad_down'], ['dpad_left', 'dpad_left'], ['dpad_right', 'dpad_right'],
  ];
  const PADDLES: [string, PadButton][] = [
    ['left_paddle1', 'lpaddle1'], ['right_paddle1', 'rpaddle1'], ['left_paddle2', 'lpaddle2'], ['right_paddle2', 'rpaddle2'],
  ];
  function cell(p: Pad, b: PadButton | 'guide' | 'misc1'): { text: string; src: string | undefined } {
    if (b === 'guide') return { text: tr('Home button'), src: buttonImage(padFamily(p.type), 'guide') };
    if (b === 'misc1') return { text: tr('Extra button'), src: undefined };
    return { text: padButtonName(p, b).text, src: image(p, b) };
  }
</script>

<section class="page">
  <header>
    <div>
      <h2>{tr('Controls')}</h2>
      <p class="muted">{tr('Every device the game can be played with: who plays with it, where the game’s buttons are on it, and the options it uses. Controllers show up here as soon as they are connected.')}</p>
    </div>
    <button class="btn ghost" onclick={reset}><Icon name="refresh" size={16} /> {tr('Restore defaults')}</button>
  </header>

  {#if app.launcher?.game.status !== 'complete'}
    <div class="note"><Icon name="info" size={18} /> {tr('These settings are saved now and used once the game is built.')}</div>
  {/if}

  <div class="card">
    <h3>{tr('Players')}</h3>
    {#each playerRows as s (s.key)}<SettingRow {s} />{/each}
  </div>

  <div class="layout">
    <div class="list">
      <button class="padcard" class:sel={!pad} onclick={() => choose('keyboard')}>
        <div class="top"><span class="pname">{tr('Keyboard and mouse')}</span></div>
        <div class="chips">
          {#if own.keyboard >= 0}
            <span class="chip who">{tr('Player {n}', { n: own.keyboard + 1 })}</span>
          {:else}
            <span class="chip warn">{tr('Not used in the game')}</span>
          {/if}
          {#if app.lastUsed === 'keyboard' || !app.pads.length}<span class="chip ok">{tr('Using it now')}</span>{/if}
        </div>
      </button>
      {#each app.pads as p (p.id)}
        <button class="padcard" class:sel={pad?.id === p.id} onclick={() => choose(p.id)}>
          <div class="top">
            <span class="pname">{padTitle(app.pads, p)}</span>
            {#if p.battery !== null}<span class="bat" class:low={p.battery < 25}>{p.battery}%</span>{/if}
          </div>
          <div class="chips">
            {#if (own.pads.get(p.id) ?? -1) >= 0}
              <span class="chip who">{tr('Player {n}', { n: (own.pads.get(p.id) ?? 0) + 1 })}</span>
            {:else}
              <span class="chip warn">{tr('Not used in the game')}</span>
            {/if}
            {#if app.usedPad?.id === p.id}<span class="chip ok">{tr('Using it now')}</span>{/if}
            <span class="chip">{padKind(p)}</span>
            {#if p.wired !== null}<span class="chip">{p.wired ? tr('Wired') : tr('Wireless')}</span>{/if}
          </div>
        </button>
      {/each}
      {#if !app.pads.length}
        <div class="card empty">
          <Icon name="controls" size={40} />
          <p>{tr('No controller connected. Connect one with a cable or pair it in your system’s Bluetooth settings: it shows up here at once.')}</p>
        </div>
      {/if}
      <p class="help">{tr('Controller support follows the connection and model. On Windows, Pro Controller 2 Bluetooth connects through the Nintendo transport included with Age of Decomp. Close other controller bridge apps and press Sync.')}</p>
    </div>

    <div class="detail">
      <div class="card">
        <div class="trow">
          <h3>{pad ? padTitle(app.pads, pad) : tr('Keyboard and mouse')}</h3>
          <div class="assign">
            <button class="btn" disabled={player === 0} onclick={() => assign(1)}>{tr('Use as player 1')}</button>
            <button class="btn" disabled={player === 1} onclick={() => assign(2)}>{tr('Use as player 2')}</button>
          </div>
        </div>
        <p class="help">
          {#if player >= 0}
            {tr('It plays as player {n}.', { n: player + 1 })}
          {:else}
            {tr('The game does not read it with the players set above.')}
          {/if}
        </p>
        {#if pad}
          <div class="chips">
            {#if pad.has_rumble}<span class="chip">{tr('Rumble')}</span>{/if}
            {#if pad.has_trigger_rumble}<span class="chip">{tr('Trigger rumble')}</span>{/if}
            {#if pad.has_gyro && pad.has_accel}<span class="chip">{tr('Motion sensors')}</span>{/if}
            {#if pad.has_paddles}<span class="chip">{tr('Back buttons')}</span>{/if}
            {#if pad.has_led}<span class="chip">{tr('led|Light')}</span>{/if}
          </div>
        {/if}
      </div>

      <div class="card">
        <h3>{tr('Buttons')}</h3>
        <p class="help">
          {#if pad && pictures === 'game'}
            {tr('The game shows the buttons of a Switch controller. This is where each one is on this controller.')}
          {:else if pad && pictures === 'device'}
            {tr('The game is made for a Switch controller. This is where each of its buttons is on this controller, and the game shows them this way.')}
          {:else if pad}
            {tr('The game is made for a Switch controller. This is where each of its buttons is on this controller.')}
          {:else if pictures === 'game'}
            {tr('The game shows the buttons of a Switch controller. These are the keys for each one.')}
          {:else if pictures === 'device'}
            {tr('The game is made for a Switch controller. These are the keys for each of its buttons, and the game shows them this way.')}
          {:else}
            {tr('The game is made for a Switch controller. These are the keys for each of its buttons.')}
          {/if}
          {(pad ? binds.customPad : binds.customKeys) ? tr('Changed in the game’s menu.') : tr('These are the defaults.')}
          {tr('To change them, open the menu in the game: Controls.')}
        </p>
        <dl class="map">
          {#if !pad}
            <dt>{tr('Left stick (move)')}</dt><dd>{keysOf(MOVE) || '—'}</dd>
            <dt>{tr('Right stick (camera)')}</dt><dd>{[keysOf(CAMERA), tr('the mouse')].filter(Boolean).join(' · ')}</dd>
          {/if}
          {#each BUTTONS as a (a)}
            <dt>{actionLabel(a)}</dt>
            <dd>
              {#if !pad}
                {keysOf([a]) || '—'}
              {:else}
                {#each buttonsOf(a) as b (b)}
                  {@const name = padButtonName(pad, b).text}
                  {@const src = image(pad, b)}
                  <span class="pb">
                    {#if src}<img width="28" height="28" {src} alt={name.length > 2 ? '' : name} />{/if}
                    {#if !src || name.length > 2}{name}{/if}
                  </span>
                {:else}—{/each}
              {/if}
            </dd>
          {/each}
          <dt>{actionLabel('MENU')}</dt>
          <dd>
            {#if !pad}
              {[keysOf(['MENU']), 'F1'].filter(Boolean).join(' · ')}
            {:else}
              {#each buttonsOf('MENU') as b (b)}<span class="pb">{padButtonName(pad, b).text}</span>{/each}
              {combo(pad, 'MINUS', 'PLUS')}
            {/if}
          </dd>
          <dt>{tr('Performance overlay')}</dt>
          <dd>{pad ? combo(pad, 'LCLICK', 'RCLICK') : 'F10'}</dd>
          {#if !pad}
            <dt>{tr('Fullscreen')}</dt><dd>F11</dd>
          {/if}
        </dl>
        {#if pad?.has_paddles && !paddleBound}
          <p class="help">{tr('The back buttons have no game button until you give them one in the game’s menu.')}</p>
        {/if}
      </div>

      {#if optionRows.length}
        <div class="card">
          <h3>{tr('Options')}</h3>
          {#if pad}<p class="help">{tr('Only what this controller has is listed. A change applies to every controller that has it.')}</p>{/if}
          {#each optionRows as s (s.key)}<SettingRow {s} />{/each}
        </div>
      {/if}

      {#if pad}
        <div class="card tester">
          <h3>{tr('Test')}</h3>
          <div class="sticks">
            {#each [['leftx', 'lefty', tr('Left stick')], ['rightx', 'righty', tr('Right stick')]] as [x, y, label] (label)}
              <div class="stick">
                <div class="ring"><span class="dot" style:left="{50 + (live?.axes[x] ?? 0) * 42}%" style:top="{50 + (live?.axes[y] ?? 0) * 42}%"></span></div>
                <span class="faint">{label}</span>
              </div>
            {/each}
            <div class="triggers">
              {#each [['left_trigger', 'zl'], ['right_trigger', 'zr']] as [k, b] (k)}
                <div class="trig"><div class="tfill" style:height="{(live?.axes[k] ?? 0) * 100}%"></div></div>
                <span class="faint tl">{padButtonName(pad, b as PadButton).text}</span>
              {/each}
            </div>
          </div>
          <div class="buttons">
            {#each [...TEST, ...(pad.has_paddles ? PADDLES : [])] as [k, b] (k)}
              <span class="b" class:on={live?.buttons[k]}>
                {#if cell(pad, b).src}<img width="40" height="40" src={cell(pad, b).src} alt={cell(pad, b).text} />{:else}{cell(pad, b).text}{/if}
              </span>
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
              <button class="btn" onclick={() => api.rumble(pad!.id, 0.35, 0.15, 400)}><Icon name="vibrate" size={16} /> {tr('rumble|Light')}</button>
              <button class="btn" onclick={() => api.rumble(pad!.id, 1, 1, 600)}><Icon name="vibrate" size={16} /> {tr('rumble|Strong')}</button>
            </div>
          {/if}
        </div>
      {/if}
    </div>
  </div>
  {#if hidden}
    <button class="btn ghost more" onclick={() => (app.showAdvanced = true)}>{tr('Show {n} advanced options', { n: hidden })}</button>
  {/if}
</section>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 1100px;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 20px;
  }
  header p {
    margin-top: 6px;
    max-width: 80ch;
  }
  /* As SettingsPage's note. */
  .note {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    border-radius: var(--radius-m);
    background: var(--focus-soft);
    color: var(--text);
  }
  .card h3 {
    margin-bottom: 4px;
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
    align-items: start;
  }
  .list,
  .detail {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  .detail {
    gap: 16px;
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
  .chip.who {
    color: var(--text);
  }
  .chip.ok {
    color: var(--ok);
    background: var(--ok-soft);
  }
  .chip.warn {
    color: var(--warn);
    background: var(--warn-soft);
  }
  .trow {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .assign {
    display: flex;
    gap: 8px;
  }
  /* As a setting row's help. */
  .help {
    margin-top: 6px;
    color: var(--text-dim);
    font-size: 13.5px;
    max-width: 62ch;
  }
  /* As System's facts: the game's button, then where it is on this device. */
  .map {
    display: grid;
    grid-template-columns: 200px 1fr;
    gap: 6px 14px;
    margin: 12px 0 0;
    align-items: center;
  }
  dt {
    color: var(--text-faint);
  }
  dd {
    margin: 0;
    min-height: 28px;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 12px;
  }
  .pb {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .tester {
    display: flex;
    flex-direction: column;
    gap: 18px;
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
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 56px;
    padding: 8px 4px;
    text-align: center;
    border-radius: 8px;
    background: var(--surface-3);
    font-size: 12px;
    font-weight: 650;
    color: var(--text-dim);
  }
  .b.on {
    background: var(--focus);
    color: var(--bg);
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
  .more {
    align-self: flex-start;
  }
  @media (max-width: 1080px) {
    .layout {
      grid-template-columns: 1fr;
    }
  }
</style>
