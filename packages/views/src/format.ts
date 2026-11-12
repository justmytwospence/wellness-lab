export const dayToDate = (day: number): Date => new Date(day * 86_400_000);
export const isoDay = (day: number): string => dayToDate(day).toISOString().slice(0, 10);
export const parseDay = (s: string): number => Math.floor(Date.parse(`${s}T00:00:00Z`) / 86_400_000);
export const WEEKDAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
/** Monday = 0 */
export const weekday = (day: number): number => (((day + 3) % 7) + 7) % 7;
export function prettyDay(day: number): string {
  return dayToDate(day).toLocaleDateString('en', { weekday: 'short', month: 'short', day: 'numeric', year: 'numeric', timeZone: 'UTC' });
}

/** Clock time from minutes after midnight (wraps), 24 h or 12 h. */
export function clock(min: number, h12 = false): string {
  if (!Number.isFinite(min)) return '';
  const m = Math.round(((min % 1440) + 1440) % 1440);
  const hh = Math.floor(m / 60), mm = m % 60;
  if (!h12) return `${String(hh).padStart(2, '0')}:${String(mm).padStart(2, '0')}`;
  return `${hh % 12 || 12}:${String(mm).padStart(2, '0')} ${hh < 12 ? 'am' : 'pm'}`;
}
/** Duration from hours, "7 h 05 m". */
export function duration(hours: number): string {
  if (!Number.isFinite(hours)) return '';
  const m = Math.round(hours * 60);
  return `${Math.floor(m / 60)} h ${String(m % 60).padStart(2, '0')} m`;
}
export function signed(v: number, digits = 1): string {
  if (!Number.isFinite(v)) return '';
  return `${v > 0 ? '+' : v < 0 ? '\u2212' : ''}${Math.abs(v).toFixed(digits)}`;
}
export const z = (v: number): string => (Number.isFinite(v) ? `${signed(v, 1)} SD` : '');
export const pct = (v: number, digits = 0): string => (Number.isFinite(v) ? `${(v * 100).toFixed(digits)}%` : '');
/** A difference of natural logs as a percent change. */
export const logPct = (d: number, digits = 1): string => (Number.isFinite(d) ? `${signed((Math.exp(d) - 1) * 100, digits)}%` : '');
export function num(v: number | null | undefined, digits = 1): string {
  return v === null || v === undefined || !Number.isFinite(v) ? '\u2013' : v.toFixed(digits);
}
