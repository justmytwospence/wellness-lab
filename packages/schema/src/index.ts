export * from './codes';
export * from './schema';

export type LabelKind =
  | 'illness' | 'vaccination' | 'alcohol' | 'travel' | 'altitude' | 'hard_training' | 'stress' | 'late_meal'
  | 'caffeine_late' | 'sauna' | 'cold_exposure' | 'medication_change' | 'menstruation' | 'poor_environment' | 'other';
export type LabelSource = 'user' | 'confirmed' | 'inferred';
export type ProposalStatus = 'open' | 'confirmed' | 'denied' | 'unsure' | 'snoozed' | 'expired';

export interface Label {
  id: string;
  kind: LabelKind;
  start_day: number;
  end_day: number;
  onset_day?: number | null;
  severity?: number | null;
  note: string;
  exclude_baseline: boolean;
  source: LabelSource;
  proposal_id?: string | null;
  created_at: string;
  updated_at: string;
  deleted_at?: string | null;
}

export interface Evidence {
  nights: number;
  d_rhr: number | null;
  d_lnrmssd: number | null;
  d_resp: number | null;
  d_temp: number | null;
  d_stress: number | null;
  d_early_late: number | null;
  p_value: number | null;
  hard_day_before: boolean;
  weekend: boolean;
  tz_overlap: boolean;
  alt_overlap: boolean;
  signals: string[];
}

export interface Proposal {
  id: string;
  kind: LabelKind;
  start_day: number;
  end_day: number;
  posterior: Partial<Record<LabelKind, number>>;
  evidence: Evidence;
  score: number;
  priority: number;
  status: ProposalStatus;
  auto: boolean;
  asked: boolean;
  created_at: string;
  answered_at?: string | null;
  notified_at?: string | null;
  snoozed_until?: string | null;
  model_version: string;
}

export interface Settings {
  question_budget_per_week: number;
  ask_min: number;
  ask_max: number;
  auto_label_travel: boolean;
  auto_label_altitude: boolean;
  cycle_tracking: boolean;
  free_days: number[];
  sleep_need_h: number | null;
  tst_bias_min: number;
  baseline_window: number;
  hrv_range_days: number;
  k_daily: number;
  k_swc: number;
  home_altitude_m: number | null;
  notify_ntfy: boolean;
  notify_webhook: boolean;
}

export const DEFAULT_SETTINGS: Settings = {
  question_budget_per_week: 2, ask_min: 0.3, ask_max: 0.95, auto_label_travel: true, auto_label_altitude: true,
  cycle_tracking: false, free_days: [5, 6], sleep_need_h: null, tst_bias_min: 30, baseline_window: 28,
  hrv_range_days: 60, k_daily: 1.5, k_swc: 0.5, home_altitude_m: null, notify_ntfy: true, notify_webhook: true,
};

export interface ExperimentDef {
  id: string; name: string; intervention: string; outcome: string; period_nights: number; washout: number;
  blocks: number; randomized: boolean; start_day: number; seed: number; higher_is_better: boolean;
}

export interface State {
  labels: Label[];
  proposals: Proposal[];
  settings: Settings;
  experiments: ExperimentDef[];
  adherence: Record<string, Record<number, boolean>>;
  revision: number;
}

export interface ManifestFile {
  path: string; sha256: string; bytes: number; rows: number; t_min?: number | null; t_max?: number | null; fingerprint?: string;
}
export interface Manifest {
  schema_version: number;
  generated_at: string;
  importer: string;
  source: { kind: string; [k: string]: unknown };
  files: ManifestFile[];
  [k: string]: unknown;
}
export type TableName = 'days' | 'sleeps' | 'stages' | 'activities' | 'tags' | 'devices';
export type SeriesName = 'hr' | 'stress' | 'body_battery' | 'resp' | 'spo2' | 'steps' | 'hrv5' | 'sleep_move' | 'altitude' | 'motion';

/** The engine serialises tables as `{n, num: {col: (number|null)[]}, txt: {col: (string|null)[]}}`. */
export interface Table { n: number; num: Record<string, (number | null)[]>; txt: Record<string, (string | null)[]> }

export type Align = 'onset' | 'clock';

export type Job =
  | { kind: 'hrv4t'; range_days?: number; k_daily?: number; k_swc?: number }
  | { kind: 'sessions' }
  | { kind: 'beats'; id: string; window_min?: number }
  | { kind: 'dfa' }
  | { kind: 'nights'; from?: number; to?: number }
  | { kind: 'night'; day: number; pad_min?: number }
  | { kind: 'downsample'; series: string; t0: number; t1: number; px: number }
  | { kind: 'raster'; metric: string; align?: Align; from: number; to: number; bin_min?: number }
  | { kind: 'typical'; metric: string; align?: Align; a: [number, number]; b?: [number, number]; bin_min?: number }
  | { kind: 'baseline'; metric: string; window?: number; k_night?: number; k_short?: number; exclude_labelled?: boolean }
  | { kind: 'circadian'; from: number; to: number }
  | { kind: 'actogram'; from: number; to: number; period_h?: number }
  | { kind: 'detect' }
  | { kind: 'event_study'; label_kind?: LabelKind; label_ids?: string[]; metrics: string[]; window?: [number, number]; controls?: { n?: number; doy?: number; same_weekday?: boolean; same_load_class?: boolean; exclude?: number }; n_boot?: number }
  | { kind: 'compare'; sel: [number, number][]; base?: [number, number][] }
  | { kind: 'bench'; metrics?: string[] }
  | { kind: 'teardown' }
  | { kind: 'dfm' }
  | { kind: 'experiment'; def: ExperimentDef; adherence?: Record<number, boolean> }
  | { kind: 'coverage' }
  | { kind: 'days'; from: number; to: number; columns: string[] }
  | { kind: 'seasonal'; from: number; to: number; source: 'days' | 'nights'; column: string }
  | { kind: 'registry' };

// Result shapes are the engine's serde output; the ones the app leans on are typed, the rest are open records.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type Json = any;
export interface RasterMeta { width: number; height: number; days: number[]; bin_min: number; span_min: number; domain: [number, number, number]; log: boolean; metric: string }
export interface Band { minute: number[]; p10: number[]; p50: number[]; p90: number[]; n: number[] }
export interface Chan { t: number[]; v: number[]; q: number[] }
export interface NightChannels {
  day: number; onset: number; wake: number; utc_offset_min: number;
  series: Record<string, Chan>; stages: Table; activities: Table; row: Record<string, number | null>; bands: Record<string, Band>;
}
export interface MethodSpec {
  id: string; name: string; group: string; summary: string; formula: string;
  params: { name: string; default: number; min: number; max: number; step: number; unit: string; help: string }[];
  inputs: string[]; min_resolution_s: number | null; validity: 'A' | 'B' | 'C' | 'D'; refs: string[];
}
export interface RefSpec { key: string; citation: string; title: string; year: number; url: string; doi: boolean }
export interface Registry { methods: MethodSpec[]; refs: RefSpec[] }

export interface Results {
  night: NightChannels;
  nights: Table;
  seasonal: Table;
  registry: Registry;
  raster: RasterMeta;
  detect: { proposals: Proposal[]; model_version: string };
  [k: string]: Json;
}
export type ResultOf<J extends Job> = Results[J['kind']];
export interface RunArrays { [name: string]: Float64Array | Float32Array | Int32Array | Uint8Array }
