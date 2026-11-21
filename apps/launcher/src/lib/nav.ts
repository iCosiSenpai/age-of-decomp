// Controller-first navigation: D-pad / left stick / arrow keys move focus spatially, A (or Enter) activates, B (or Esc)
// goes back, LB/RB (PageUp/PageDown) switch pages, Start (or F5) jumps to Play. Gamepad input is turned into the same keyboard
// events so every control behaves the same. Two gamepad sources: the backend's SDL 3 (`pad-nav` events; every pad
// the game supports, Switch 2 included) and, in a plain browser, the WebView Gamepad API.
// Spatial focus logic adapted from the maintainer's Deluxe8 launcher.

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

type Dir = 'up' | 'down' | 'left' | 'right';
export type NavButton = 'up' | 'down' | 'left' | 'right' | 'a' | 'b' | 'x' | 'y' | 'lb' | 'rb' | 'start' | 'back';

function visible(el: HTMLElement): boolean {
  const r = el.getBoundingClientRect();
  if (r.width === 0 || r.height === 0) return false;
  const style = getComputedStyle(el);
  return style.visibility !== 'hidden' && style.display !== 'none';
}

function scopeRoot(): ParentNode {
  const modals = document.querySelectorAll<HTMLElement>('[data-nav-scope]');
  return modals.length ? modals[modals.length - 1] : document;
}

function candidates(): HTMLElement[] {
  return [...scopeRoot().querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (el) => visible(el) && !el.closest('[data-nav-skip]'),
  );
}

/** Pick the nearest focusable element in `dir`, favouring alignment over raw distance. */
export function moveFocus(dir: Dir): boolean {
  const all = candidates();
  const current = document.activeElement as HTMLElement | null;
  if (!current || current === document.body || !all.includes(current)) {
    // The page's data-autofocus control only while it can take focus: a disabled Play falls through to the first one.
    const first = all.find((el) => el.hasAttribute('data-autofocus')) ?? all[0];
    first?.focus();
    return !!first;
  }
  const a = current.getBoundingClientRect();
  const ax = a.left + a.width / 2;
  const ay = a.top + a.height / 2;
  let best: HTMLElement | null = null;
  let bestScore = Infinity;
  for (const el of all) {
    if (el === current) continue;
    const b = el.getBoundingClientRect();
    const dx = b.left + b.width / 2 - ax;
    const dy = b.top + b.height / 2 - ay;
    let main: number;
    let cross: number;
    switch (dir) {
      case 'up':
        if (b.bottom > a.top + 2) continue;
        main = a.top - b.bottom;
        cross = Math.abs(dx);
        break;
      case 'down':
        if (b.top < a.bottom - 2) continue;
        main = b.top - a.bottom;
        cross = Math.abs(dx);
        break;
      case 'left':
        if (b.right > a.left + 2) continue;
        main = a.left - b.right;
        cross = Math.abs(dy);
        break;
      case 'right':
        if (b.left < a.right - 2) continue;
        main = b.left - a.right;
        cross = Math.abs(dy);
        break;
    }
    const score = Math.max(0, main) + cross * 2.2;
    if (score < bestScore) {
      bestScore = score;
      best = el;
    }
  }
  if (best) {
    best.focus();
    best.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    return true;
  }
  return false;
}

function isTextEntry(el: Element | null): boolean {
  if (!el) return false;
  if (el instanceof HTMLTextAreaElement) return true;
  if (el instanceof HTMLInputElement) return ['text', 'search', 'password', 'number', 'url', 'email'].includes(el.type);
  return false;
}

/** Elements that consume left/right themselves (sliders, segmented controls). */
function consumesHorizontal(el: Element | null): boolean {
  return !!el && (el.matches('input[type="range"]') || !!el.closest('[data-horizontal]'));
}

export interface NavHandlers {
  back: () => void;
  tab: (delta: number) => void;
  start: () => void;
}

export function installKeyboardNav(h: NavHandlers) {
  window.addEventListener('keydown', (e) => {
    if (e.defaultPrevented) return;
    const el = document.activeElement;
    const map: Record<string, Dir> = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'left', ArrowRight: 'right' };
    const dir = map[e.key];
    if (dir) {
      if (isTextEntry(el) && (dir === 'left' || dir === 'right')) return;
      if (consumesHorizontal(el) && (dir === 'left' || dir === 'right')) return;
      if (moveFocus(dir)) e.preventDefault();
      return;
    }
    if (e.key === 'Escape') {
      h.back();
      e.preventDefault();
    } else if (e.key === 'PageUp') {
      h.tab(-1);
      e.preventDefault();
    } else if (e.key === 'PageDown') {
      h.tab(1);
      e.preventDefault();
    } else if (e.key === 'F5') {
      // The keyboard's Start: the hint bar shows it as Play.
      h.start();
      e.preventDefault();
    }
  });
}

function fireKey(key: string) {
  const target = (document.activeElement as HTMLElement | null) ?? document.body;
  target.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
}

/** One logical button press from any controller source. */
export function pressNav(b: NavButton, h: NavHandlers) {
  switch (b) {
    case 'up':
      return fireKey('ArrowUp');
    case 'down':
      return fireKey('ArrowDown');
    case 'left':
      return fireKey('ArrowLeft');
    case 'right':
      return fireKey('ArrowRight');
    case 'a': {
      const el = document.activeElement as HTMLElement | null;
      if (el && el !== document.body) el.click();
      else moveFocus('down');
      return;
    }
    case 'b':
      return h.back();
    case 'lb':
      return h.tab(-1);
    case 'rb':
      return h.tab(1);
    case 'start':
      return h.start();
  }
}

/** Key repeat for held directions, shared by both gamepad sources. */
export class Repeater {
  private held = new Map<NavButton, number>();
  private static DELAY = 360;
  private static RATE = 110;
  constructor(private h: NavHandlers) {}
  set(b: NavButton, down: boolean, now = performance.now()) {
    const since = this.held.get(b);
    if (!down) {
      this.held.delete(b);
      return;
    }
    if (since === undefined) {
      this.held.set(b, now);
      pressNav(b, this.h);
    }
  }
  tick(now = performance.now()) {
    for (const [b, since] of this.held) {
      if (!['up', 'down', 'left', 'right'].includes(b)) continue;
      if (now - since > Repeater.DELAY) {
        this.held.set(b, now - Repeater.DELAY + Repeater.RATE);
        pressNav(b, this.h);
      }
    }
  }
}

/** WebView Gamepad API (standard mapping) — used only when the backend's SDL input is not available. */
export function installWebGamepadNav(h: NavHandlers, connected: (name: string | null) => void, enabled: () => boolean) {
  const rep = new Repeater(h);
  let lastName: string | null = null;
  const loop = (now: number) => {
    if (enabled()) {
      const pads = navigator.getGamepads?.() ?? [];
      const pad = [...pads].find((p) => p && p.connected) ?? null;
      const name = pad ? pad.id : null;
      if (name !== lastName) {
        lastName = name;
        connected(name);
      }
      if (pad && document.hasFocus()) {
        const b = (i: number) => !!pad.buttons[i]?.pressed;
        const ax = pad.axes[0] ?? 0;
        const ay = pad.axes[1] ?? 0;
        rep.set('up', b(12) || ay < -0.6, now);
        rep.set('down', b(13) || ay > 0.6, now);
        rep.set('left', b(14) || ax < -0.6, now);
        rep.set('right', b(15) || ax > 0.6, now);
        rep.set('a', b(0), now);
        rep.set('b', b(1), now);
        rep.set('lb', b(4), now);
        rep.set('rb', b(5), now);
        rep.set('start', b(9), now);
        rep.tick(now);
      }
    }
    requestAnimationFrame(loop);
  };
  requestAnimationFrame(loop);
}
