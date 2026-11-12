export interface ModelFit { level: number; level_var: number; beta: number; tau: number; g: number; sigma_eta: number; sigma_eps: number }

/** Tonight's expected ln RMSSD for a planned load, with an 80 % interval, and as the notebook's score (20 x ln RMSSD). */
export function whatIf(m: ModelFit, load: number) {
  const decay = Math.exp(-1 / m.tau);
  const g1 = decay * m.g + load / 100;
  const mean = m.level + m.beta * g1;
  const sd = Math.sqrt(m.level_var + m.sigma_eta ** 2 + m.sigma_eps ** 2);
  return { mean, lo80: Math.exp(mean - 1.2816 * sd), hi80: Math.exp(mean + 1.2816 * sd), rmssd: Math.exp(mean), score: 20 * mean };
}
