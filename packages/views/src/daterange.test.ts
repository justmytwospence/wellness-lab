import { describe, expect, it } from 'vitest';
import { parseRange } from './daterange';
import { isoDay, parseDay } from './format';

const today = parseDay('2026-11-04');
const r = (s: string) => parseRange(s, today)?.map(isoDay);

describe('parseRange', () => {
  it('reads relative spans', () => {
    expect(r('last 90 days')).toEqual(['2026-08-07', '2026-11-04']);
    expect(r('90d')).toEqual(['2026-08-07', '2026-11-04']);
    expect(r('2 weeks')).toEqual(['2026-10-22', '2026-11-04']);
  });
  it('reads years, seasons, months and intervals', () => {
    expect(r('2025')).toEqual(['2025-01-01', '2025-12-31']);
    expect(r('summer 2024')).toEqual(['2024-06-21', '2024-09-22']);
    expect(r('Mar 2024')).toEqual(['2024-03-01', '2024-03-31']);
    expect(r('2024-03 to 2024-06')).toEqual(['2024-03-01', '2024-06-30']);
    expect(r('since 2023-05-01')).toEqual(['2023-05-01', '2026-11-04']);
    expect(r('2024-03-01 to 2024-04-15')).toEqual(['2024-03-01', '2024-04-15']);
  });
  it('declines nonsense', () => expect(parseRange('whenever', today)).toBeNull());
});
