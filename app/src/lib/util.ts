export const LAB_EMPTY = 0;
export const isTouch = () => matchMedia('(max-width: 767px)').matches;
/** A numeric column of a night table as plain numbers (NaN for missing). */
export const col = (t: { n: number; num: Record<string, (number | null)[]> } | null | undefined, name: string): number[] =>
  (t?.num[name] ?? []).map((v) => (v === null ? NaN : v));
export const txt = (t: { n: number; txt: Record<string, (string | null)[]> } | null | undefined, name: string): (string | null)[] => t?.txt[name] ?? [];
export const finite = (v: number | null | undefined): v is number => v !== null && v !== undefined && Number.isFinite(v);
