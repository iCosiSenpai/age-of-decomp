// Application state shared by the pages (Svelte 5 runes).

import { api } from './api';
import { padFamily, type PadFamily } from './controller-glyphs';
import catalogJson from './settings/catalog.json';
import { resolveLocale, setLocale } from './i18n.svelte';
import type { Catalog, CatalogSetting, DlssStatus, LauncherState, NvidiaFile, PageId, Pad, PrepareEvent, SettingValues, SystemInfo, UpdateInfo } from './types';

export const catalog = catalogJson as unknown as Catalog;

class AppState {
  update = $state<{ checking: boolean; installing: boolean; progress: number; info: UpdateInfo | null; error: string | null }>({
    checking: false, installing: false, progress: 0, info: null, error: null,
  });
  page = $state<PageId>('home');
  launcher = $state<LauncherState | null>(null);
  system = $state<SystemInfo | null>(null);
  settings = $state<SettingValues>({});
  pads = $state<Pad[]>([]);
  /** What the player used last: a pad's id, 'keyboard' (a key or a mouse button), or null before any input. */
  lastUsed = $state<number | 'keyboard' | null>(null);
  showAdvanced = $state(false);
  toasts = $state<{ id: number; text: string; tone: 'ok' | 'warn' | 'err' | 'info' }[]>([]);
  prepare = $state<{ running: boolean; stage: number; fraction: number; text: string; elapsed: number; eta: number | null; log: string[]; result: 'done' | 'error' | 'cancelled' | null; error: string | null }>({
    running: false, stage: 0, fraction: 0, text: '', elapsed: 0, eta: null, log: [], result: null, error: null,
  });
  game = $state<{ running: boolean; lastExit: number | null; fps: number | null }>({ running: false, lastExit: null, fps: null });
  /** NVIDIA's DLSS file for Upscaling = NVIDIA DLSS (src-tauri/src/dlss.rs): its status, the download or the check of a file the player
   *  chose, the download's progress and the error. Kept here so the Display page shows them again after a page
   *  change. */
  dlss = $state<{ status: DlssStatus | null; downloading: boolean; checking: boolean; progress: number; error: string | null }>({
    status: null, downloading: false, checking: false, progress: 0, error: null,
  });
  /** The same for NVIDIA's frame generation file (Frame generation): its own download, check, progress and error. The
   *  status is `dlss.status`, which says of both files whether they are there. */
  fg = $state<{ downloading: boolean; checking: boolean; progress: number; error: string | null }>({
    downloading: false, checking: false, progress: 0, error: null,
  });

  /** The pad in use: the one used last, or the first one before any input. Null with the keyboard and mouse. */
  get usedPad(): Pad | null {
    if (this.lastUsed === 'keyboard') return null;
    return this.pads.find((p) => p.id === this.lastUsed) ?? this.pads[0] ?? null;
  }
  /** The controller family that drives the button hints: the device in use. */
  get hintPad(): PadFamily {
    const pad = this.usedPad;
    return pad ? padFamily(pad.type) : 'keyboard';
  }

  private toastId = 0;
  // Not reactive on purpose: the Display page's $effect calls loadDlss, and a reactive flag would run that effect again
  // each time a failed read clears it.
  private dlssLoading = false;

  async init() {
    this.launcher = await api.state();
    setLocale(resolveLocale(this.launcher.ui.language, this.launcher.system_locale));
    api.onUpdateProgress((fraction) => (this.update.progress = fraction));
    api.onDlssProgress((fraction) => (this.dlss.progress = fraction));
    api.onFgProgress((fraction) => (this.fg.progress = fraction));
    if (this.launcher.ui.check_updates) void this.checkUpdates();
    if (this.launcher.game.status !== 'complete') this.page = 'setup';
    const params = new URLSearchParams(location.search);
    const page = params.get('page') as PageId | null;
    if (page) this.page = page;
    api.systemInfo().then((s) => (this.system = s)).catch(() => {});
    api.settings().then((s) => (this.settings = s)).catch(() => {});
    api.pads().then((p) => this.setPads(p)).catch(() => {});
    api.onPads((p) => this.setPads(p));
    api.onPadActive((id) => (this.lastUsed = id));
    // A real key or mouse button hands the hints to the keyboard. The keys the controller navigation makes up are
    // not trusted events, so they do not count.
    const keyboard = (e: Event) => {
      if (e.isTrusted) this.lastUsed = 'keyboard';
    };
    for (const type of ['keydown', 'pointerdown', 'wheel']) window.addEventListener(type, keyboard, { capture: true, passive: true });
    api.onPrepare((e) => this.onPrepare(e));
    // The build is looked at again each time the window comes back: the game file may have moved, the build may
    // have been made or removed outside this window.
    window.addEventListener('focus', () => {
      if (!this.prepare.running) api.state().then((s) => (this.launcher = s)).catch(() => {});
    });
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
    // The pad in use was unplugged: back to the first one left, or to the keyboard.
    if (typeof this.lastUsed === 'number' && !p.some((x) => x.id === this.lastUsed)) this.lastUsed = null;
  }

  async checkUpdates() {
    if (this.update.checking || this.update.installing) return;
    this.update.checking = true;
    this.update.error = null;
    try {
      this.update.info = await api.checkUpdates();
      this.update.error = this.update.info.previous_error;
    } catch (e) {
      this.update.info = null;
      this.update.error = String(e);
    } finally { this.update.checking = false; }
  }

  async setDiagnostics(enabled: boolean) {
    try {
      this.launcher = await api.setDiagnostics(enabled);
    } catch (e) {
      this.toast(String(e), 'err');
    }
  }

  async installUpdate() {
    if (this.update.installing || this.prepare.running || this.game.running) return;
    this.update.installing = true;
    this.update.progress = 0;
    this.update.error = null;
    try { await api.installUpdate(); }
    catch (e) { this.update.error = String(e); this.update.installing = false; }
  }

  /** Reads the DLSS file's status the first time the Display page needs it; a download or a file check replaces it. */
  async loadDlss() {
    if (this.dlss.status || this.dlss.downloading || this.dlss.checking || this.dlssLoading) return;
    this.dlssLoading = true;
    try { this.dlss.status = await api.dlssStatus(); }
    catch { /* no status: the page shows no DLSS block */ }
    finally { this.dlssLoading = false; }
  }

  /** Downloads one of NVIDIA's files (the DLSS file, or the frame generation one); the Display page calls it once the
   *  player has accepted NVIDIA's licence. */
  async downloadDlss(kind: NvidiaFile = 'dlss') {
    const job = this[kind];
    if (job.downloading || job.checking) return;
    job.downloading = true;
    job.progress = 0;
    job.error = null;
    try { this.dlss.status = await api.dlssDownload(kind); }
    catch (e) { job.error = String(e); }
    finally { job.downloading = false; }
  }

  /** Checks a copy of one of NVIDIA's files the player chose and keeps it when it is NVIDIA's. */
  async pickDlss(file: string, kind: NvidiaFile = 'dlss') {
    const job = this[kind];
    if (job.downloading || job.checking) return;
    job.checking = true;
    job.error = null;
    try { this.dlss.status = await api.dlssPick(file, kind); }
    catch (e) { job.error = String(e); }
    finally { job.checking = false; }
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

  /** A setting greyed out because its `when` condition does not hold, or, for HDR output and its brightness values,
   *  because no screen of this PC shows HDR now (the game would draw its usual picture). */
  inactive(s: CatalogSetting): boolean {
    if (this.launcher?.hdr_screen === false && (s.key === 'hdr' || s.when?.hdr !== undefined)) return true;
    if (!s.when) return false;
    return Object.entries(s.when).some(([k, v]) => {
      const dep = catalog.settings.find((x) => x.key === k);
      return (this.settings[k] ?? dep?.default ?? '') !== v;
    });
  }

  async set(key: string, value: string) {
    const before = this.settings;
    this.settings = { ...this.settings, [key]: value };
    try {
      this.settings = await api.setSetting(key, value);
    } catch (err) {
      // What the file holds is what the game reads: show that again, not the value that was not saved.
      this.settings = await api.settings().catch(() => before);
      this.toast(String(err), 'err');
    }
  }

  /** These settings back to their defaults; `done` is said when it worked, the reason when it did not. */
  async reset(keys: string[], done: string) {
    try {
      this.settings = await api.resetSettings(keys);
      this.toast(done, 'ok');
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
  { id: 'performance', label: 'Performance', icon: 'performance', advanced: true },
  { id: 'diagnostics', label: 'Diagnostics', icon: 'diagnostics', advanced: true },
  { id: 'system', label: 'System', icon: 'system' },
];
