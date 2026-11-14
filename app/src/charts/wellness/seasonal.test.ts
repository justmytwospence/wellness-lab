import { describe, expect, it } from 'vitest';
import type { Table } from '@wl/schema';
import { byYear, niceScale, yearColor } from './seasonalMath';
import { viridis } from './trendcharts';

describe('byYear', () => {
  const t: Table = {
    n: 4,
    num: { day: [1, 2, 3, 4], year: [2025, 2025, 2024, 2024], doy: [61, 62, 61, 100], dev: [1.5, 2, -0.5, 3] },
    txt: {},
  };
  it('groups by year, ascending, indexed by day of the 366-day calendar', () => {
    const lines = byYear(t);
    expect(lines.map((l) => l.year)).toEqual([2024, 2025]);
    expect(lines[0]!.dev[61]).toBe(-0.5);
    expect(lines[0]!.dev[100]).toBe(3);
    expect(lines[1]!.dev[61]).toBe(1.5);
    expect(lines[1]!.dev[62]).toBe(2);
    expect(Number.isNaN(lines[0]!.dev[62])).toBe(true);
    expect(Number.isNaN(lines[0]!.dev[0])).toBe(true);
    expect(lines[0]!.dev).toHaveLength(367);
  });
  it('bridges 29 February in a non-leap year only', () => {
    const g: Table = { n: 4, num: { year: [2025, 2025, 2024, 2024], doy: [59, 61, 59, 61], dev: [1, 3, 1, 3] }, txt: {} };
    const [a, b] = byYear(g);
    expect(a!.dev[60]).toBeNaN();
    expect(b!.dev[60]).toBe(2);
  });
});

describe('niceScale', () => {
  it('rounds to one significant figure', () => {
    expect(niceScale(0.27)).toBe(0.3);
    expect(niceScale(2.4)).toBe(2);
    expect(niceScale(7.6)).toBe(8);
  });
  it('falls back to 1', () => {
    expect(niceScale(0)).toBe(1);
    expect(niceScale(NaN)).toBe(1);
  });
});

describe('yearColor', () => {
  it('runs from a visible violet to yellow', () => {
    expect(yearColor(0, 1)).toBe(viridis(1));
    expect(yearColor(0, 3)).toBe(viridis(0.25));
    expect(yearColor(2, 3)).toBe(viridis(1));
  });
});
