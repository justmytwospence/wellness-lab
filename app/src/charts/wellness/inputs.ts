import { useJob } from '../../lib/useJob';

/** Which input series the loaded data source actually has, from the engine's coverage job. */
export function useInputs() {
  const { data } = useJob({ kind: 'coverage' });
  const series = (data?.series ?? {}) as Record<string, number[]>;
  const has = (name: string) => (series[name] ?? []).some((v) => v > 0);
  return {
    ready: !!data,
    has,
    /** the first of `names` the data source lacks (null when it has them all, or coverage is still loading) */
    missing: (...names: string[]): string | null => (data ? (names.find((n) => !has(n)) ?? null) : null),
  };
}
