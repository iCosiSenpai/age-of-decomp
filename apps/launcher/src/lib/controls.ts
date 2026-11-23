// What the game's input layer does with the connected devices and the saved settings, mirrored from the engine so the
// Controls page shows what the game will do, not a guess: runtime/input.c (assign: who plays with what),
// runtime/input_map.c (the button layout, the default keys and buttons, and the bind_ lines the game menu's
// remapping writes) and runtime/hud_prompts.c (whose buttons the game's own pictures show).
// Change both sides together.

import { tr } from './i18n.svelte';
import type { Pad, SettingValues } from './types';

export const PLAYERS = 2;

/** The value a player's device setting takes for a pad: `pad:<SDL GUID>:<k>`, k counting the pads of that GUID from 0
 *  (runtime/input.c pad_value). */
export function padValue(pads: Pad[], pad: Pad): string {
  return `pad:${pad.guid}:${pads.filter((p) => p.guid === pad.guid).indexOf(pad)}`;
}

/** A pad's name as the game's menu writes it: with `#n` when several pads of its model are connected. */
export function padTitle(pads: Pad[], pad: Pad): string {
  const same = pads.filter((p) => p.guid === pad.guid);
  return same.length > 1 ? `${pad.name} #${same.indexOf(pad) + 1}` : pad.name;
}

/** The connected pad a saved device value names: the exact one, or by the rule for values written by launchers
 *  up to 0.1.2, which counted the pads of one model from 1 (a value naming one past the last pad of its model is
 *  that last pad). Null when none. The same two steps as `owners` below, for one value alone. */
export function padOfValue(pads: Pad[], value: string): Pad | null {
  if (!value.startsWith('pad:')) return null;
  const values = pads.map((p) => padValue(pads, p));
  const exact = values.indexOf(value);
  if (exact >= 0) return pads[exact];
  const stem = value.slice(0, value.lastIndexOf(':') + 1);
  for (let i = pads.length - 1; i >= 0; i--) {
    if (!values[i].startsWith(stem)) continue;
    const later = pads.slice(i + 1).some((q) => q.guid === pads[i].guid);
    if (!later && `${stem}${Number(values[i].slice(stem.length)) + 1}` === value) return pads[i];
  }
  return null;
}

/** The player (0 or 1) each device plays as in the game, -1 when the game does not read it. */
export interface Owners {
  keyboard: number;
  pads: Map<number, number>;
}

/** runtime/input.c assign(). */
export function owners(pads: Pad[], settings: SettingValues): Owners {
  const dev = Array.from({ length: PLAYERS }, (_, p) => settings[`player${p + 1}_device`] || 'auto');
  const values = pads.map((p) => padValue(pads, p));
  const owner = pads.map(() => -1);
  let keyboard = -1;
  for (let p = 0; p < PLAYERS; p++) {
    if (dev[p] === 'keyboard' && keyboard < 0) keyboard = p;
    if (!dev[p].startsWith('pad:')) continue;
    let found = values.findIndex((v, i) => owner[i] < 0 && v === dev[p]);
    // Launchers up to 0.1.2 counted the pads of one model from 1: a value that names one past the last connected pad
    // of its model is that last pad.
    const stem = dev[p].slice(0, dev[p].lastIndexOf(':') + 1);
    for (let i = pads.length - 1; i >= 0 && found < 0; i--) {
      if (owner[i] >= 0 || !values[i].startsWith(stem)) continue;
      const later = pads.slice(i + 1).some((q) => q.guid === pads[i].guid);
      if (!later && `${stem}${Number(values[i].slice(stem.length)) + 1}` === dev[p]) found = i;
    }
    if (found >= 0) owner[found] = p;
  }
  for (let p = 0; p < PLAYERS; p++) {
    if (dev[p] !== 'auto') continue;
    const free = owner.findIndex((o) => o < 0);
    if (free >= 0) owner[free] = p;
  }
  if (keyboard < 0 && dev[0] !== 'none') keyboard = 0;
  return { keyboard, pads: new Map(pads.map((p, i) => [p.id, owner[i]])) };
}

/** Which face button is the game's A: `nintendo` the right one (B bottom, Y left, X top), `xbox` the bottom one
 *  (B right, X left, Y top). */
export type Layout = 'nintendo' | 'xbox';

/** runtime/hud_prompts.c plan_make (button_prompts): whose buttons the game's own pictures show. `device`: the
 *  device in use, its bindings and the keys; `maker`: one maker's buttons whatever the controller is, and the game's own
 *  without a controller; `game`: always the game's own Switch buttons. Any other value is Automatic, as in the game. */
export function gamePictures(settings: SettingValues): 'device' | 'maker' | 'game' {
  const setting = (settings.button_prompts || 'auto').toLowerCase();
  if (setting === 'game') return 'game';
  return setting === 'xbox' || setting === 'playstation' || setting === 'nintendo' ? 'maker' : 'device';
}

/** runtime/input_map.c input_pad_layout_pick: every button where a Switch controller has it, on every maker's pad,
 *  whatever an older settings file says (the Xbox layout, A at the bottom, is gone in 0.2.0). */
export function padLayout(_settings: SettingValues): Layout {
  return 'nintendo';
}

/** The game's buttons: the Switch controller the game reads, the stick directions the keys drive, and Age of Decomp's
 *  own menu. The names are the engine's (input_map.c action_names); a bind line is `bind_<name>` in lower case. */
export const ACTIONS = [
  'A', 'B', 'X', 'Y', 'LCLICK', 'RCLICK', 'L', 'R', 'ZL', 'ZR', 'PLUS', 'MINUS', 'LEFT', 'UP', 'RIGHT', 'DOWN',
  'LSTICK_LEFT', 'LSTICK_UP', 'LSTICK_RIGHT', 'LSTICK_DOWN', 'RSTICK_LEFT', 'RSTICK_UP', 'RSTICK_RIGHT', 'RSTICK_DOWN',
  'MENU',
] as const;
export type Action = (typeof ACTIONS)[number];

/** A pad's inputs by position (input_map.c pad_names). */
export const PAD_BUTTONS = [
  'south', 'east', 'west', 'north', 'lstick', 'rstick', 'l', 'r', 'start', 'back', 'dpad_left', 'dpad_up', 'dpad_right',
  'dpad_down', 'zl', 'zr', 'lpaddle1', 'rpaddle1', 'lpaddle2', 'rpaddle2',
] as const;
export type PadButton = (typeof PAD_BUTTONS)[number];

// input_map.c key_names
const KEYS = new Set([
  ...'abcdefghijklmnopqrstuvwxyz0123456789',
  'space', 'tab', 'lshift', 'lctrl', 'up', 'down', 'left', 'right', 'mouse_l', 'mouse_r', 'mouse_m', 'mouse_x1', 'mouse_x2',
  'enter', 'backspace', 'rshift', 'rctrl', 'lalt', 'capslock', 'insert', 'delete', 'home', 'end', 'pageup', 'pagedown',
  'f2', 'f3', 'f4', 'f5', 'f6', 'f7', 'f9', 'f12',
  'num0', 'num1', 'num2', 'num3', 'num4', 'num5', 'num6', 'num7', 'num8', 'num9',
  'num_add', 'num_sub', 'num_mul', 'num_div', 'num_decimal',
  'minus', 'equals', 'lbracket', 'rbracket', 'semicolon', 'apostrophe', 'comma', 'period', 'slash', 'backslash', 'grave',
  'escape',
]);

// input_map.c DEFAULT_KEYS
const DEFAULT_KEYS: [string, Action][] = [
  ['w', 'LSTICK_UP'], ['a', 'LSTICK_LEFT'], ['s', 'LSTICK_DOWN'], ['d', 'LSTICK_RIGHT'],
  ['up', 'RSTICK_UP'], ['left', 'RSTICK_LEFT'], ['down', 'RSTICK_DOWN'], ['right', 'RSTICK_RIGHT'],
  ['mouse_l', 'Y'], ['mouse_r', 'ZL'], ['z', 'ZL'], ['f', 'ZR'], ['x', 'ZR'],
  ['space', 'X'], ['lshift', 'B'], ['e', 'A'], ['q', 'L'], ['r', 'R'],
  ['lctrl', 'LCLICK'], ['c', 'RCLICK'],
  ['1', 'LEFT'], ['2', 'UP'], ['3', 'RIGHT'], ['4', 'DOWN'],
  ['tab', 'PLUS'], ['m', 'MINUS'], ['escape', 'MENU'],
];

// input_map.c DEFAULT_PADS: by position; only the face buttons differ between the layouts. The menu has no pad button
// by default.
const REST: [PadButton, Action][] = [
  ['lstick', 'LCLICK'], ['rstick', 'RCLICK'], ['l', 'L'], ['r', 'R'], ['start', 'PLUS'], ['back', 'MINUS'],
  ['dpad_left', 'LEFT'], ['dpad_up', 'UP'], ['dpad_right', 'RIGHT'], ['dpad_down', 'DOWN'], ['zl', 'ZL'], ['zr', 'ZR'],
];
const DEFAULT_PADS: Record<Layout, [PadButton, Action][]> = {
  nintendo: [['south', 'B'], ['east', 'A'], ['west', 'Y'], ['north', 'X'], ...REST],
  xbox: [['south', 'A'], ['east', 'B'], ['west', 'X'], ['north', 'Y'], ...REST],
};

/** INPUT_IS_BUTTON: the stick directions take keys only. */
const takesPad = (a: Action) => !a.includes('STICK_');

export interface Bindings {
  keys: Map<Action, string[]>;
  pad: Map<Action, PadButton[]>;
  /** A bind_ line of the player left some key, or some pad button of this layout, other than the default. */
  customKeys: boolean;
  customPad: boolean;
}

/** A player's keys and, in `layout`, pad buttons for every game button: the defaults with the player's bind_ lines
 *  over them (input_map.c input_map_bind_into: a line replaces that button's bindings; `key:default` / `pad:default`
 *  keep that part's defaults; a part the line does not name is left without input). */
export function bindings(settings: SettingValues, player: number, layout: Layout): Bindings {
  const keys = new Map<Action, string[]>();
  const pad = new Map<Action, PadButton[]>();
  let customKeys = false;
  let customPad = false;
  const differs = (a: string[], b: string[]) => a.length !== b.length || a.some((x) => !b.includes(x));
  for (const a of ACTIONS) {
    const defKeys = DEFAULT_KEYS.filter(([, x]) => x === a).map(([k]) => k);
    const defPad = DEFAULT_PADS[layout].filter(([, x]) => x === a).map(([b]) => b);
    keys.set(a, defKeys);
    pad.set(a, defPad);
    const line = settings[`${player === 1 ? 'p2_' : ''}bind_${a.toLowerCase()}`];
    if (!line) continue;
    const k: string[] = [];
    const p: PadButton[] = [];
    let none = false;
    let keyDefault = false;
    let padDefault = false;
    for (const raw of line.split(',')) {
      const tok = raw.trim().toLowerCase();
      if (tok === 'none') none = true;
      else if (tok.startsWith('pad:') && takesPad(a)) {
        const name = tok.slice(4).trim();
        if (name === 'default') padDefault = true;
        else if ((PAD_BUTTONS as readonly string[]).includes(name)) p.push(name as PadButton);
      } else if (tok.startsWith('key:')) {
        const name = tok.slice(4).trim();
        if (name === 'default') keyDefault = true;
        else if (KEYS.has(name)) k.push(name);
      }
    }
    if (!k.length && !p.length && !none && !keyDefault && !padDefault) continue; // no valid entry: the defaults stay
    keys.set(a, [...(keyDefault ? defKeys : []), ...k]);
    pad.set(a, [...(padDefault ? defPad : []), ...p]);
    customKeys ||= differs(keys.get(a)!, defKeys);
    customPad ||= differs(pad.get(a)!, defPad);
  }
  return { keys, pad, customKeys, customPad };
}

/** A key as the game menu names it. Names printed on the key itself (Tab, Esc, F5, Num 3) are the same in every
 *  language and stay as they are. The punctuation keys are named by the character a US keyboard has there: the
 *  game's menu shows the character of the player's own layout, which the launcher does not know. */
export function keyLabel(key: string): string {
  switch (key) {
    case 'space': return tr('Space');
    case 'tab': return 'Tab';
    case 'lshift': return tr('Left Shift');
    case 'rshift': return tr('Right Shift');
    case 'lctrl': return tr('Left Ctrl');
    case 'rctrl': return tr('Right Ctrl');
    case 'lalt': return 'Alt';
    case 'up': return tr('Up arrow');
    case 'down': return tr('Down arrow');
    case 'left': return tr('Left arrow');
    case 'right': return tr('Right arrow');
    case 'mouse_l': return tr('Left mouse button');
    case 'mouse_r': return tr('Right mouse button');
    case 'mouse_m': return tr('Middle mouse button');
    case 'mouse_x1': return tr('Mouse button 4');
    case 'mouse_x2': return tr('Mouse button 5');
    case 'enter': return tr('Enter');
    case 'backspace': return 'Backspace';
    case 'capslock': return tr('Caps Lock');
    case 'insert': return tr('key|Ins');
    case 'delete': return tr('key|Del');
    case 'home': return 'Home';
    case 'end': return tr('key|End');
    case 'pageup': return tr('Page Up');
    case 'pagedown': return tr('Page Down');
    case 'escape': return 'Esc';
    case 'minus': return '-';
    case 'equals': return '=';
    case 'lbracket': return '[';
    case 'rbracket': return ']';
    case 'semicolon': return ';';
    case 'apostrophe': return "'";
    case 'comma': return ',';
    case 'period': return '.';
    case 'slash': return '/';
    case 'backslash': return '\\';
    case 'grave': return '`';
    case 'num_add': return 'Num +';
    case 'num_sub': return 'Num -';
    case 'num_mul': return 'Num *';
    case 'num_div': return 'Num /';
    case 'num_decimal': return 'Num .';
  }
  if (key.startsWith('num')) return `Num ${key.slice(3)}`;
  return key.toUpperCase(); // letters, digits, F keys
}

/** The maker a pad's button names follow. */
export type Maker = 'xbox' | 'playstation' | 'nintendo' | 'generic';

/** runtime/input.c input_pads: Nintendo also for a pad of another model whose right button is marked A. The
 *  maker's own button names (Menu, View, Options, L1, ZL) are what is printed on the pad: never translated. */
export function padMaker(pad: Pad): Maker {
  if (pad.type.startsWith('xbox')) return 'xbox';
  if (pad.type.startsWith('ps')) return 'playstation';
  if (pad.type.startsWith('switch') || pad.type.startsWith('joycon') || pad.type === 'gamecube') return 'nintendo';
  return pad.gen2 || pad.labels[1] === 'a' ? 'nintendo' : 'generic';
}

/** The maker's name of the kind of pad, for its card. */
export function padKind(pad: Pad): string {
  if (pad.type.startsWith('ps')) return 'PlayStation';
  if (pad.type.startsWith('xbox')) return 'Xbox';
  if (pad.type === 'gamecube') return 'GameCube';
  if (pad.type.startsWith('switch') || pad.type.startsWith('joycon')) return pad.gen2 ? 'Nintendo Switch 2' : 'Nintendo Switch';
  return tr('Generic');
}

const FACE: Record<string, string> = { a: 'A', b: 'B', x: 'X', y: 'Y', cross: '✕', circle: '○', square: '□', triangle: '△' };
/** The picture of a face label in assets/buttons/<family>: the PlayStation shapes are stored under a b x y. */
const FACE_IMAGE: Record<string, string> = { a: 'a', b: 'b', x: 'x', y: 'y', cross: 'a', circle: 'b', square: 'x', triangle: 'y' };

/** A pad's button as its maker names it (the same names as the game menu, runtime/ui/binds_page.cpp pad_label), and
 *  the name of its picture in assets/buttons/<family> where there is one. */
export function padButtonName(pad: Pad, button: PadButton): { text: string; image: string | null } {
  const maker = padMaker(pad);
  const face = ['south', 'east', 'west', 'north'].indexOf(button);
  if (face >= 0) {
    const label = pad.labels[face];
    if (label) return { text: FACE[label], image: FACE_IMAGE[label] };
    return { text: [tr('Bottom button'), tr('Right button'), tr('Left button'), tr('Top button')][face], image: null };
  }
  const image: Partial<Record<PadButton, string>> = {
    l: 'lb', r: 'rb', zl: 'lt', zr: 'rt', lstick: 'ls', rstick: 'rs', start: 'start', back: 'back',
    dpad_up: 'up', dpad_down: 'down', dpad_left: 'left', dpad_right: 'right',
  };
  const img = image[button] ?? null;
  const named: Partial<Record<Maker, Partial<Record<PadButton, string>>>> = {
    xbox: { l: 'LB', r: 'RB', zl: 'LT', zr: 'RT', lstick: 'LS', rstick: 'RS', start: 'Menu', back: 'View' },
    playstation: {
      l: 'L1', r: 'R1', zl: 'L2', zr: 'R2', lstick: 'L3', rstick: 'R3',
      start: pad.type === 'ps3' ? 'Start' : 'Options',
      back: pad.type === 'ps5' ? 'Create' : pad.type === 'ps4' ? 'Share' : 'Select',
    },
    nintendo: { l: 'L', r: 'R', zl: 'ZL', zr: 'ZR', start: '+', back: '−' },
  };
  const own = named[maker]?.[button];
  if (own) return { text: own, image: img };
  switch (button) {
    case 'lstick': return { text: tr('Left stick press'), image: img };
    case 'rstick': return { text: tr('Right stick press'), image: img };
    case 'l': return { text: tr('Left shoulder'), image: img };
    case 'r': return { text: tr('Right shoulder'), image: img };
    case 'zl': return { text: tr('Left trigger'), image: img };
    case 'zr': return { text: tr('Right trigger'), image: img };
    case 'start': return { text: 'Start', image: img };
    case 'back': return { text: tr('pad|Back'), image: img };
    case 'dpad_up': return { text: tr('D-pad up'), image: img };
    case 'dpad_down': return { text: tr('D-pad down'), image: img };
    case 'dpad_left': return { text: tr('D-pad left'), image: img };
    case 'dpad_right': return { text: tr('D-pad right'), image: img };
    case 'lpaddle1': return { text: tr('Left back button'), image: null };
    case 'rpaddle1': return { text: tr('Right back button'), image: null };
    case 'lpaddle2': return { text: tr('Left back button 2'), image: null };
    case 'rpaddle2': return { text: tr('Right back button 2'), image: null };
  }
  return { text: button, image: null };
}

/** The game's button as the game shows it: the letters and signs of a Switch controller. */
export function actionLabel(a: Action): string {
  switch (a) {
    case 'PLUS': return '+';
    case 'MINUS': return '−';
    case 'LCLICK': return tr('Left stick press');
    case 'RCLICK': return tr('Right stick press');
    case 'UP': return tr('D-pad up');
    case 'DOWN': return tr('D-pad down');
    case 'LEFT': return tr('D-pad left');
    case 'RIGHT': return tr('D-pad right');
    case 'MENU': return tr("Age of Decomp's menu");
  }
  return a; // A B X Y L R ZL ZR
}
