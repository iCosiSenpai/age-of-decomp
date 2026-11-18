export type PageId =
  | 'home'
  | 'setup'
  | 'display'
  | 'picture'
  | 'game'
  | 'audio'
  | 'controls'
  | 'pads'
  | 'performance'
  | 'diagnostics'
  | 'system';

export type UiLanguage = 'auto' | 'en' | 'it';
export type BuildStatus = 'none' | 'incomplete' | 'complete';

export interface CheckStep {
  id: string;
  ok: boolean;
  detail: string;
}

/** aod-nup profile::Report */
export interface NupReport {
  ok: boolean;
  title_id: string;
  versions: number[];
  has_base: boolean;
  has_update: boolean;
  addons: number;
  build_id: string | null;
  romfs_files: number;
  romfs_bytes: number;
  steps: CheckStep[];
}

export interface GpuInfo {
  index: number;
  name: string;
  vendor: string;
  vram_gb: number;
  discrete: boolean;
}

export interface SystemInfo {
  os: string;
  cpu: string;
  threads: number;
  avx2: boolean;
  ram_gb: number;
  gpus: GpuInfo[];
  vulkan: string | null; // loader version, null when no Vulkan loader was found
}

export interface DiskInfo {
  path: string;
  free_gb: number;
  total_gb: number;
}

export interface GameState {
  nup: string | null;
  update_nup: string | null;
  dlc_nup: string | null;
  output: string;
  status: BuildStatus;
  built_at: string | null;
  nup_present: boolean;
}

export interface UiPrefs {
  language: UiLanguage;
  close_on_play: boolean;
  check_updates: boolean;
  jobs: number;
  rate_unlock: boolean;
}

export interface UpdateInfo {
  current: string;
  latest: string | null;
  available: boolean;
  previous_error: string | null;
}

export interface LauncherState {
  version: string;
  engine_dir: string | null;
  data_dir: string;
  game: GameState;
  ui: UiPrefs;
  system_locale: { language: string; country: string };
  recomplauncher: boolean;
}

export type PrepareKind = 'stage' | 'progress' | 'log' | 'done' | 'error' | 'cancelled';

export interface PrepareEvent {
  kind: PrepareKind;
  stage?: number; // 1-based index into PREPARE_STAGES
  fraction?: number; // 0..1 overall
  text?: string;
  line?: string;
  elapsed_s?: number;
  eta_s?: number | null;
}

export type PadType =
  | 'xbox360'
  | 'xboxone'
  | 'ps3'
  | 'ps4'
  | 'ps5'
  | 'switch_pro'
  | 'joycon_left'
  | 'joycon_right'
  | 'joycon_pair'
  | 'gamecube'
  | 'standard'
  | 'unknown';

export interface Pad {
  id: number;
  guid: string;
  name: string;
  type: PadType;
  /** Switch 2 controllers report the Switch Pro / Joy-Con type in SDL; `gen2` tells them apart. */
  gen2: boolean;
  has_gyro: boolean;
  has_accel: boolean;
  has_rumble: boolean;
  has_trigger_rumble: boolean;
  has_led: boolean;
  battery: number | null;
  wired: boolean | null;
}

export interface PadState {
  buttons: Record<string, boolean>;
  axes: Record<string, number>;
  gyro: [number, number, number] | null;
  accel: [number, number, number] | null;
}

export type GameEvent = { kind: 'started' } | { kind: 'exited'; code: number | null } | { kind: 'perf'; fps: number };

export type SettingValues = Record<string, string>;

export interface CatalogChoice {
  value: string;
  label: string;
}

export interface CatalogSetting {
  key: string;
  section: string;
  group: string;
  kind: 'choice' | 'toggle' | 'slider' | 'dynamic';
  label: string;
  default: string;
  help: string;
  restart?: boolean;
  advanced?: boolean;
  when?: Record<string, string>;
  choices?: CatalogChoice[];
  min?: number;
  max?: number;
  step?: number;
  format?: string;
  unit?: string;
  source?: 'gpus' | 'pads';
}

export interface Catalog {
  file: string;
  groups: { id: string; label: string; advanced?: boolean }[];
  settings: CatalogSetting[];
}
