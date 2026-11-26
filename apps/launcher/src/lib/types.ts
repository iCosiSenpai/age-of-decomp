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

export type UiLanguage = 'auto' | 'en' | 'it' | 'zh';
export type BuildStatus = 'none' | 'incomplete' | 'stale' | 'complete';

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
  vulkan13: boolean; // the card's own Vulkan version is 1.3 or later
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
  /** The engine in this launcher is not the one the game was built with: a rebuild is offered, the build still plays. */
  engine_changed: boolean;
}

export interface UiPrefs {
  language: UiLanguage;
  close_on_play: boolean;
  check_updates: boolean;
  jobs: number;
  rate_unlock: boolean;
}

/** NVIDIA's own files in the standalone compiler (src-tauri/src/dlss.rs): the DLSS file for NVIDIA DLSS (`ready`) and
 *  the frame generation file (`fg_ready`). */
export interface DlssStatus {
  ready: boolean;
  fg_ready: boolean;
  rtx: boolean;
  /** One of this PC's cards is of a series frame generation runs on (RTX 40 and later). */
  fg_card: boolean;
  version: string;
  license_url: string;
}

/** Which of NVIDIA's two files: DLSS itself, or frame generation. */
export type NvidiaFile = 'dlss' | 'fg';

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
  diagnostics_available: boolean;
  diagnostics: boolean | null;
  /** A screen of this PC shows HDR now; null when the system does not say (system.rs hdr_screen). */
  hdr_screen: boolean | null;
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
  /** Back buttons (paddles). */
  has_paddles: boolean;
  /** What is printed on the south, east, west and north buttons, as SDL reports it: a b x y, cross circle square
   *  triangle, or '' when it does not know. */
  labels: string[];
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
