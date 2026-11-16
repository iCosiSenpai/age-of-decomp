// Application state shared by the pages (Svelte 5 runes).

import { api } from './api';
import catalogJson from './settings/catalog.json';
import { resolveLocale, setLocale } from './i18n.svelte';
import type { Catalog, CatalogSetting, LauncherState, PageId, Pad, PrepareEvent, SettingValues, SystemInfo } from './types';

export const catalog = catalogJson as unknown as Catalog;

class AppState {
  page = $state<PageId>('home');
  launcher = $state<LauncherState | null>(null);
  system = $state<SystemInfo | null>(null);
  settings = $state<SettingValues>({});
  pads = $state<Pad[]>([]);
  /** The controller family that drives the button hints at the bottom. */
  hintPad = $state<'xbox' | 'playstation' | 'nintendo' | 'keyboard'>('keyboard');
  showAdvanced = $state(false);
  toasts = $state<{ id: number; text: string; tone: 'ok' | 'warn' | 'err' | 'info' }[]>([]);
  prepare = $state<{ running: boolean; stage: number; fraction: number; text: string; elapsed: number; eta: number | null; log: string[]; result: 'done' | 'error' | 'cancelled' | null; error: string | null }>({
    running: false, stage: 0, fraction: 0, text: '', elapsed: 0, eta: null, log: [], result: null, error: null,
  });
  game = $state<{ running: boolean; lastExit: number | null; fps: number | null }>({ running: false, lastExit: null, fps: null });

  private toastId = 0;

  async init() {
    this.launcher = await api.state();
    setLocale(resolveLocale(this.launcher.ui.language, this.launcher.system_locale));
    if (this.launcher.game.status !== 'complete') this.page = 'setup';
    const params = new URLSearchParams(location.search);
    const page = params.get('page') as PageId | null;
    if (page) this.page = page;
    api.systemInfo().then((s) => (this.system = s)).catch(() => {});
    api.settings().then((s) => (this.settings = s)).catch(() => {});
    api.pads().then((p) => this.setPads(p)).catch(() => {});
    api.onPads((p) => this.setPads(p));
    api.onPrepare((e) => this.onPrepare(e));
    api.onGame((e) => {
      if (e.kind === 'started') this.game = { running: true, lastExit: null, fps: null };
      else if (e.kind === 'exited') {
        this.game = { running: false, lastExit: e.code, fps: null };
        api.settings().then((s) => (this.settings = s)); // the in-game menu may have changed hwaoi.ini
      } else if (e.kind === 'perf') this.game.fps = e.fps;
    });
  }

  setPads(p: Pad[]) {
    this.pads = p;
    const first = p[0];
    if (!first) return;
    this.hintPad = first.type.startsWith('ps') ? 'playstation' : first.type.startsWith('xbox') ? 'xbox' :
      first.type === 'standard' || first.type === 'unknown' ? 'xbox' : 'nintendo';
  }

  onPrepare(e: PrepareEvent) {
    const p = this.prepare;
    if (e.kind === 'progress' || e.kind === 'stage') {
      p.running = true;
      if (e.stage) p.stage = e.stage;
      if (e.fraction !== undefined) p.fraction = e.fraction;
      if (e.text) p.text = e.text;
      if (e.elapsed_s !== undefined) p.elapsed = e.elapsed_s;
      p.eta = e.eta_s ?? null;
    } else if (e.kind === 'log' && e.line) {
      p.log = [...p.log.slice(-199), e.line];
    } else if (e.kind === 'done') {
      p.running = false;
      p.fraction = 1;
      p.result = 'done';
      api.state().then((s) => (this.launcher = s));
      api.settings().then((s) => (this.settings = s));
    } else if (e.kind === 'error') {
      p.running = false;
      p.result = 'error';
      p.error = e.text ?? null;
    } else if (e.kind === 'cancelled') {
      p.running = false;
      p.result = 'cancelled';
    }
  }

  value(s: CatalogSetting): string {
    return this.settings[s.key] ?? s.default;
  }

  /** A setting greyed out because its `when` condition does not hold. */
  inactive(s: CatalogSetting): boolean {
    if (!s.when) return false;
    return Object.entries(s.when).some(([k, v]) => {
      const dep = catalog.settings.find((x) => x.key === k);
      return (this.settings[k] ?? dep?.default ?? '') !== v;
    });
  }

  async set(key: string, value: string) {
    this.settings = { ...this.settings, [key]: value };
    try {
      this.settings = await api.setSetting(key, value);
    } catch (err) {
      this.toast(String(err), 'err');
    }
  }

  toast(text: string, tone: 'ok' | 'warn' | 'err' | 'info' = 'info') {
    const id = ++this.toastId;
    this.toasts = [...this.toasts, { id, text, tone }];
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), 4200);
  }
}

export const app = new AppState();

export const NAV: { id: PageId; label: string; icon: string; advanced?: boolean; needsBuild?: boolean }[] = [
  { id: 'home', label: 'Play', icon: 'play' },
  { id: 'setup', label: 'Setup', icon: 'setup' },
  { id: 'display', label: 'Display', icon: 'display' },
  { id: 'picture', label: 'Picture', icon: 'picture' },
  { id: 'game', label: 'Game', icon: 'game' },
  { id: 'audio', label: 'Audio', icon: 'audio' },
  { id: 'controls', label: 'Controls', icon: 'controls' },
  { id: 'pads', label: 'Controllers', icon: 'pads' },
  { id: 'performance', label: 'Performance', icon: 'performance', advanced: true },
  { id: 'diagnostics', label: 'Diagnostics', icon: 'diagnostics', advanced: true },
  { id: 'system', label: 'System', icon: 'system' },
];
