import type { Table } from '@wl/schema';

/** The night-table metrics the analysis views offer, with plain names and the data input each one needs. */
export const METRICS: { id: string; label: string; input: string; log?: boolean }[] = [
  { id: 'hr_min30', label: 'Lowest heart rate (30 min)', input: 'hr' },
  { id: 'hr_mean', label: 'Mean sleeping heart rate', input: 'hr' },
  { id: 'hr_nadir', label: 'Heart-rate nadir', input: 'hr' },
  { id: 'nadir_frac', label: 'Nadir position in the night', input: 'hr' },
  { id: 'hr_decline', label: 'Heart-rate decline', input: 'hr' },
  { id: 'dip_pct', label: 'Heart-rate dip', input: 'hr' },
  { id: 'ln_rmssd', label: 'Overnight HRV (ln RMSSD)', input: 'hrv5' },
  { id: 'ln_mean', label: 'Overnight HRV mean (ln)', input: 'hrv5' },
  { id: 'rmssd_4h', label: 'HRV, first 4 h', input: 'hrv5', log: true },
  { id: 'rmssd_norm', label: 'HRV normalised to heart rate', input: 'hrv5' },
  { id: 'garmin_hrv', label: "Garmin's overnight HRV", input: 'days.hrv_nightly', log: true },
  { id: 'sleep_stress', label: 'Sleep stress', input: 'stress' },
  { id: 'rest_frac', label: 'Rest share of the night', input: 'stress' },
  { id: 'resp_mean', label: 'Breathing rate', input: 'resp' },
  { id: 'skin_temp', label: 'Skin temperature', input: 'skin_temp' },
  { id: 'tst_h', label: 'Sleep time', input: 'sleeps' },
  { id: 'eff', label: 'Sleep efficiency', input: 'sleeps' },
  { id: 'deep_min', label: 'Deep sleep (minutes)', input: 'stages' },
  { id: 'rem_min', label: 'REM sleep (minutes)', input: 'stages' },
  { id: 'waso_min', label: 'Awake after sleep onset', input: 'stages' },
  { id: 'sleep_score', label: 'Garmin sleep score', input: 'days.sleep_score' },
  { id: 'bb_change', label: 'Body Battery charge', input: 'body_battery' },
  { id: 'tr_score', label: 'Training readiness', input: 'days.tr_score' },
];

const BY_ID = new Map(METRICS.map((m) => [m.id, m]));
export const metricLabel = (id: string): string => BY_ID.get(id)?.label ?? id.replace(/_/g, ' ');
export const metricInput = (id: string): string => BY_ID.get(id)?.input ?? id;

/** The input a metric needs when the night table holds no value for it at all, else null. */
export function missingInput(nights: Table | null | undefined, id: string): string | null {
  if (!nights || nights.n === 0) return null;
  const v = nights.num[id];
  if (!v) return metricInput(id);
  return v.some((x) => x !== null && Number.isFinite(x)) ? null : metricInput(id);
}

/** Plain numbers for display: a few significant digits whatever the magnitude. */
export function fmt(v: number | null | undefined, digits = 2): string {
  if (v === null || v === undefined || !Number.isFinite(v)) return '\u2013';
  const a = Math.abs(v);
  if (a >= 1000) return v.toFixed(0);
  if (a >= 100) return v.toFixed(1);
  if (a >= 1) return v.toFixed(digits);
  if (a === 0) return '0';
  return v.toFixed(Math.min(4, digits + 1));
}

export const nn = (a: (number | null | undefined)[]): (number | null)[] => a.map((v) => (v === null || v === undefined || !Number.isFinite(v) ? null : v));

/** Consecutive days of a sorted list as inclusive ranges. */
export function toRanges(days: number[]): [number, number][] {
  const out: [number, number][] = [];
  for (const d of [...days].sort((a, b) => a - b)) {
    const last = out[out.length - 1];
    if (last && d === last[1] + 1) last[1] = d;
    else if (!last || d > last[1]) out.push([d, d]);
  }
  return out;
}
