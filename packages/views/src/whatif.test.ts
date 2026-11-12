import { expect, it } from 'vitest';
import { whatIf } from './whatif';

it('matches the hand computation', () => {
  const m = { level: 4, level_var: 0.01, beta: -0.1, tau: 2, g: 1, sigma_eta: 0.02, sigma_eps: 0.1 };
  const w = whatIf(m, 100);
  const g1 = Math.exp(-0.5) * 1 + 1;
  expect(w.mean).toBeCloseTo(4 - 0.1 * g1, 10);
  const sd = Math.sqrt(0.01 + 0.0004 + 0.01);
  expect(w.lo80).toBeCloseTo(Math.exp(w.mean - 1.2816 * sd), 10);
  expect(w.score).toBeCloseTo(20 * w.mean, 10);
});
