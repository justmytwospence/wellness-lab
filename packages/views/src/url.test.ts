import { expect, it } from 'vitest';
import { formatLoc, formatSel, parseLoc, parseSel } from './url';

it('round trips and omits defaults', () => {
  const loc = { path: '/hrv', query: { range: '60', k: '1.5', tab: 'baseline' } };
  const s = formatLoc(loc, { range: '60', tab: 'baseline' });
  expect(s).toBe('/hrv?k=1.5');
  expect(parseLoc(s)).toEqual({ path: '/hrv', query: { k: '1.5' } });
  expect(formatSel(parseSel('10-12,20'))).toBe('10-12,20');
});
