import type { Evidence, Proposal } from '@wl/schema';
import { kindInfo } from '@wl/schema';

export const confidence = (p: number): 'likely' | 'possible' | 'unclear' => (p >= 0.7 ? 'likely' : p >= 0.4 ? 'possible' : 'unclear');

const f = (v: number | null, unit: string, digits = 1) => (v === null || !Number.isFinite(v) ? null : `${v > 0 ? '+' : v < 0 ? '\u2212' : ''}${Math.abs(v).toFixed(digits)}${unit}`);

/** The sentence that says why, the same wording the notifications use. */
export function why(e: Evidence): string {
  const bits = [
    f(e.d_rhr, ' bpm sleeping heart rate'),
    e.d_lnrmssd === null || !Number.isFinite(e.d_lnrmssd) ? null : `${(Math.exp(e.d_lnrmssd) - 1) * 100 > 0 ? '+' : '\u2212'}${Math.abs((Math.exp(e.d_lnrmssd) - 1) * 100).toFixed(0)}% HRV`,
    f(e.d_resp, ' breaths/min'),
    f(e.d_temp, ' \u00b0C skin temperature', 2),
    f(e.d_stress, ' sleep stress', 0),
  ].filter(Boolean);
  const ctx = [e.hard_day_before && 'after a hard day', e.weekend && 'on a weekend', e.tz_overlap && 'during a time zone change', e.alt_overlap && 'at altitude'].filter(Boolean);
  const nights = `${e.nights} night${e.nights === 1 ? '' : 's'}`;
  return `${bits.length ? bits.join(', ') : 'Unusual signals'} versus your usual, over ${nights}${ctx.length ? ` (${ctx.join(', ')})` : ''}.`;
}

export const title = (p: Proposal): string => `${kindInfo(p.kind).name}?`;
