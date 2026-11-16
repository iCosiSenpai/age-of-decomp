// Backend calls. Inside Tauri they go to the Rust commands in src-tauri/src/lib.rs; in a plain browser (`pnpm dev`,
// `?demo=1`) they return sample data so the interface can be reviewed without the game or the engine.

import type {
  DiskInfo,
  GameEvent,
  LauncherState,
  NupReport,
  Pad,
  PadState,
  PrepareEvent,
  SettingValues,
  SystemInfo,
  UiPrefs,
} from './types';

type Unlisten = () => void;

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) return demo(cmd, args) as Promise<T>;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(cmd, args);
}

async function listen<T>(event: string, cb: (payload: T) => void): Promise<Unlisten> {
  if (!isTauri) {
    demoListeners[event] = (p) => cb(p as T);
    return () => delete demoListeners[event];
  }
  const { listen } = await import('@tauri-apps/api/event');
  return listen<T>(event, (e) => cb(e.payload));
}

export const PREPARE_STAGES = [
  { id: 'check', label: 'Check the game files', minutes: 0.2 },
  { id: 'exefs', label: 'Extract the game code', minutes: 0.2 },
  { id: 'romfs', label: 'Prepare the game resources', minutes: 6 },
  { id: 'dlc', label: 'Install DLC', minutes: 0.1 },
  { id: 'generate', label: 'Translate the ARM64 code to C', minutes: 12 },
  { id: 'compile', label: 'Compile for your PC', minutes: 35 },
  { id: 'finish', label: 'Shaders and settings', minutes: 0.3 },
] as const;

export const api = {
  state: () => invoke<LauncherState>('get_state'),
  setUi: (patch: Partial<UiPrefs>) => invoke<LauncherState>('set_ui', { patch }),
  systemInfo: () => invoke<SystemInfo>('system_info'),
  diskInfo: (path: string) => invoke<DiskInfo>('disk_info', { path }),
  findNups: () => invoke<string[]>('find_nups'),
  pickFile: async (title: string): Promise<string | null> => {
    if (!isTauri) return 'D:\\Games\\Hyrule_Warriors_Age_of_Imprisonment.nup';
    const { open } = await import('@tauri-apps/plugin-dialog');
    const r = await open({ title, multiple: false, directory: false, filters: [{ name: 'NUP', extensions: ['nup'] }] });
    return typeof r === 'string' ? r : null;
  },
  pickFolder: async (title: string): Promise<string | null> => {
    if (!isTauri) return 'D:\\Games\\Age of Decomp';
    const { open } = await import('@tauri-apps/plugin-dialog');
    const r = await open({ title, multiple: false, directory: true });
    return typeof r === 'string' ? r : null;
  },
  inspectNup: (path: string, update: string | null) => invoke<NupReport>('inspect_nup', { path, update }),
  setGame: (nup: string, update: string | null, dlc: string | null, output: string) =>
    invoke<LauncherState>('set_game', { nup, update, dlc, output }),
  startPrepare: () => invoke<void>('start_prepare'),
  cancelPrepare: () => invoke<void>('cancel_prepare'),
  onPrepare: (cb: (e: PrepareEvent) => void) => listen<PrepareEvent>('prepare', cb),
  settings: () => invoke<SettingValues>('get_settings'),
  setSetting: (key: string, value: string) => invoke<SettingValues>('set_setting', { key, value }),
  resetSettings: (keys: string[]) => invoke<SettingValues>('reset_settings', { keys }),
  pads: () => invoke<Pad[]>('list_pads'),
  onPads: (cb: (pads: Pad[]) => void) => listen<Pad[]>('pads', cb),
  padState: (id: number) => invoke<PadState | null>('pad_state', { id }),
  rumble: (id: number, low: number, high: number, ms: number) => invoke<boolean>('pad_rumble', { id, low, high, ms }),
  play: () => invoke<void>('play'),
  onGame: (cb: (e: GameEvent) => void) => listen<GameEvent>('game', cb),
  openFolder: (kind: 'game' | 'saves' | 'logs' | 'data' | 'engine') => invoke<void>('open_folder', { kind }),
  logTail: (kind: 'prepare' | 'game') => invoke<string>('log_tail', { kind }),
  licenses: () => invoke<string>('licenses'),
  windowAction: async (action: 'minimize' | 'maximize' | 'close') => {
    if (!isTauri) return;
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    const w = getCurrentWindow();
    if (action === 'minimize') await w.minimize();
    else if (action === 'maximize') await w.toggleMaximize();
    else await w.close();
  },
};

// ── demo data ───────────────────────────────────────────────────────────────────────────────────────────────────────

const demoListeners: Record<string, (payload: unknown) => void> = {};
const params = typeof location !== 'undefined' ? new URLSearchParams(location.search) : new URLSearchParams();
const demoBuilt = params.get('built') !== '0';

const demoState: LauncherState = {
  version: '0.1.0',
  engine_dir: 'C:\\Games\\AgeOfDecomp\\engine',
  data_dir: 'C:\\Users\\Player\\AppData\\Local\\AgeOfDecomp',
  game: {
    nup: demoBuilt ? 'D:\\Games\\Hyrule_Warriors_Age_of_Imprisonment.nup' : null,
    update_nup: null,
    dlc_nup: null,
    output: 'D:\\Games\\Age of Decomp',
    status: demoBuilt ? 'complete' : 'none',
    built_at: demoBuilt ? '2026-11-16T19:42:00Z' : null,
    nup_present: true,
  },
  ui: { language: 'auto', close_on_play: false, jobs: 0, rate_unlock: false },
  system_locale: { language: 'it', country: 'IT' },
  recomplauncher: false,
};

let demoSettings: SettingValues = { output_res: '1920x1080', fps_limit: '60', render_scale: '100' };

const demoPads: Pad[] = [
  { id: 1, guid: '050000007e0500006920000001000000', name: 'Nintendo Switch 2 Pro Controller', type: 'switch_pro', gen2: true,
    has_gyro: true, has_accel: true, has_rumble: true, has_trigger_rumble: false, has_led: true, battery: 80, wired: false },
  { id: 2, guid: '030000004c050000e60c000000000000', name: 'DualSense Wireless Controller', type: 'ps5', gen2: false,
    has_gyro: true, has_accel: true, has_rumble: true, has_trigger_rumble: false, has_led: true, battery: 55, wired: true },
];

async function demo(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
  await new Promise((r) => setTimeout(r, 120));
  switch (cmd) {
    case 'get_state':
      return structuredClone(demoState);
    case 'set_ui':
      Object.assign(demoState.ui, args?.patch);
      return structuredClone(demoState);
    case 'system_info':
      return {
        os: 'Windows 11 Pro 26300', cpu: 'Intel Core Ultra 7 265K', threads: 20, avx2: true, ram_gb: 63.3,
        gpus: [{ index: 0, name: 'NVIDIA GeForce RTX 5070 Ti', vendor: 'NVIDIA', vram_gb: 16, discrete: true }],
        vulkan: '1.4.363',
      } satisfies SystemInfo;
    case 'disk_info':
      return { path: String(args?.path ?? ''), free_gb: 106.2, total_gb: 239 } satisfies DiskInfo;
    case 'find_nups':
      return ['D:\\Games\\Hyrule_Warriors_Age_of_Imprisonment.nup'];
    case 'inspect_nup':
      return {
        ok: true, title_id: '04003F40232DE000', versions: [0, 196608], has_base: true, has_update: true, addons: 0,
        build_id: '659cb288b5c5b2b49853add616c0d5bf4ea4e1ba', romfs_files: 2153, romfs_bytes: 46030886148,
        steps: [
          { id: 'title', ok: true, detail: 'Hyrule Warriors: Age of Imprisonment (04003F40232DE000)' },
          { id: 'version', ok: true, detail: 'update NCM 196608' },
          { id: 'base', ok: true, detail: 'base game present' },
          { id: 'exefs', ok: true, detail: '5 modules match' },
          { id: 'romfs', ok: true, detail: '2153 files, 42.9 GiB' },
        ],
      } satisfies NupReport;
    case 'set_game':
      demoState.game.nup = String(args?.nup);
      demoState.game.output = String(args?.output);
      return structuredClone(demoState);
    case 'start_prepare':
      demoPrepare();
      return;
    case 'cancel_prepare':
      demoCancelled = true;
      return;
    case 'get_settings':
      return { ...demoSettings };
    case 'set_setting':
      demoSettings = { ...demoSettings, [String(args?.key)]: String(args?.value) };
      return { ...demoSettings };
    case 'reset_settings':
      for (const k of (args?.keys as string[]) ?? []) delete demoSettings[k];
      return { ...demoSettings };
    case 'list_pads':
      return demoPads;
    case 'pad_state': {
      const t = Date.now() / 600;
      return {
        buttons: { south: Math.sin(t) > 0.6, east: false, west: false, north: Math.cos(t) > 0.8, left_shoulder: false,
          right_shoulder: false, back: false, start: false, guide: false, left_stick: false, right_stick: false,
          dpad_up: false, dpad_down: false, dpad_left: false, dpad_right: false },
        axes: { leftx: Math.sin(t) * 0.7, lefty: Math.cos(t) * 0.7, rightx: 0, righty: 0, left_trigger: 0.2, right_trigger: 0.8 },
        gyro: [Math.sin(t) * 0.5, 0, 0], accel: [0, -9.8, 0],
      } satisfies PadState;
    }
    case 'pad_rumble':
      return true;
    case 'play':
      setTimeout(() => demoListeners['game']?.({ kind: 'started' }), 300);
      setTimeout(() => demoListeners['game']?.({ kind: 'exited', code: 0 }), 4000);
      return;
    case 'log_tail':
      return '[demo] no log in the browser preview';
    case 'licenses':
      return 'Age of Decomp — GPL-3.0-or-later (demo)';
    default:
      return null;
  }
}

let demoCancelled = false;
function demoPrepare() {
  demoCancelled = false;
  const emit = (e: PrepareEvent) => demoListeners['prepare']?.(e);
  const total = 140;
  let i = 0;
  const tick = () => {
    if (demoCancelled) {
      emit({ kind: 'cancelled' });
      return;
    }
    const fraction = i / total;
    const stage = Math.min(PREPARE_STAGES.length, 1 + Math.floor(fraction * PREPARE_STAGES.length));
    emit({ kind: 'progress', stage, fraction, text: PREPARE_STAGES[stage - 1].label, elapsed_s: i * 8, eta_s: (total - i) * 8 });
    if (i % 7 === 0) emit({ kind: 'log', line: `[demo] step ${i}: ${PREPARE_STAGES[stage - 1].label}` });
    if (i++ >= total) {
      emit({ kind: 'done', fraction: 1 });
      demoState.game.status = 'complete';
      return;
    }
    setTimeout(tick, 90);
  };
  tick();
}
