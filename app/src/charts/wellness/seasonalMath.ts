import type { Table } from '@wl/schema';
import { col } from '../../lib/util';
import { viridis } from './trendcharts';

/** One year's deviations by day of the 366-day calendar (index 1..366; index 0 unused; NaN where there is no value). */
export interface YearLine { year: number; dev: number[] }

const isLeap = (y: number) => y % 4 === 0 && (y % 100 !== 0 || y % 400 === 0);

export function byYear(t: Table): YearLine[] {
  const year = col(t, 'year'), doy = col(t, 'doy'), dev = col(t, 'dev');
  const by = new Map<number, number[]>();
  year.forEach((y, i) => {
    if (!Number.isFinite(y) || !Number.isFinite(doy[i]) || !Number.isFinite(dev[i])) return;
    const a = by.get(y) ?? by.set(y, new Array<number>(367).fill(NaN)).get(y)!;
    a[doy[i]!] = dev[i]!;
  });
  return [...by.entries()].sort((a, b) => a[0] - b[0]).map(([y, a]) => {
    // a non-leap year has no 29 February: bridge it so the line does not break there
    if (!isLeap(y) && Number.isFinite(a[59]) && Number.isFinite(a[61])) a[60] = (a[59]! + a[61]!) / 2;
    return { year: y, dev: a };
  });
}

/** A value rounded to one significant figure (the radial scale's unit); 1 when there is nothing to scale. */
export function niceScale(v: number): number {
  if (!Number.isFinite(v) || v <= 0) return 1;
  const e = Math.floor(Math.log10(v));
  return Number((Math.round(v / 10 ** e) * 10 ** e).toPrecision(1));
}

/** Oldest to newest, from a visible violet (viridis 0 is nearly the background) to yellow. */
export const yearColor = (i: number, n: number): string => viridis(0.25 + 0.75 * (n > 1 ? i / (n - 1) : 1));

