import { expect, it } from 'vitest';
import { confidence, why } from './evidence';

it('words', () => {
  expect(confidence(0.75)).toBe('likely');
  expect(confidence(0.5)).toBe('possible');
  expect(confidence(0.1)).toBe('unclear');
  const s = why({ nights: 2, d_rhr: 5, d_lnrmssd: -0.2, d_resp: null, d_temp: null, d_stress: null, d_early_late: null, p_value: null, hard_day_before: true, weekend: false, tz_overlap: false, alt_overlap: false, signals: [] });
  expect(s).toContain('+5.0 bpm');
  expect(s).toContain('\u221218% HRV');
  expect(s).toContain('after a hard day');
});
