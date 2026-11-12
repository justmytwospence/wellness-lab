import type { LabelKind } from './index';

/** Garmin sleep stage codes in the wellness-v1 `stages` table. */
export const STAGES: { code: number; name: string; color: string }[] = [
  { code: 0, name: 'Deep', color: '#4c6fff' },
  { code: 1, name: 'Light', color: '#7aa2ff' },
  { code: 2, name: 'REM', color: '#b892ff' },
  { code: 3, name: 'Awake', color: '#ffb547' },
  { code: 4, name: 'Unmeasured', color: '#4b5568' },
];

export const LABEL_KINDS: { kind: LabelKind; name: string; exclude: boolean; color: string; icon: string }[] = [
  { kind: 'illness', name: 'Illness', exclude: true, color: '#ff6b6b', icon: 'thermometer' },
  { kind: 'vaccination', name: 'Vaccination', exclude: true, color: '#ff9a5c', icon: 'syringe' },
  { kind: 'alcohol', name: 'Alcohol', exclude: false, color: '#ffb547', icon: 'wine' },
  { kind: 'travel', name: 'Travel', exclude: true, color: '#7aa2ff', icon: 'plane' },
  { kind: 'altitude', name: 'Altitude', exclude: true, color: '#b892ff', icon: 'mountain' },
  { kind: 'hard_training', name: 'Hard training', exclude: false, color: '#ff6b6b', icon: 'dumbbell' },
  { kind: 'stress', name: 'Stress', exclude: false, color: '#ff9a5c', icon: 'brain' },
  { kind: 'late_meal', name: 'Late meal', exclude: false, color: '#e8c36a', icon: 'utensils' },
  { kind: 'caffeine_late', name: 'Late caffeine', exclude: false, color: '#c08a5b', icon: 'coffee' },
  { kind: 'sauna', name: 'Sauna', exclude: false, color: '#ff7a59', icon: 'flame' },
  { kind: 'cold_exposure', name: 'Cold exposure', exclude: false, color: '#5ad1ff', icon: 'snowflake' },
  { kind: 'medication_change', name: 'Medication change', exclude: true, color: '#9ad17a', icon: 'pill' },
  { kind: 'menstruation', name: 'Menstruation', exclude: false, color: '#ff7aa8', icon: 'droplet' },
  { kind: 'poor_environment', name: 'Poor environment', exclude: false, color: '#8892a6', icon: 'bed' },
  { kind: 'other', name: 'Other', exclude: false, color: '#8892a6', icon: 'tag' },
];

export const kindInfo = (k: LabelKind) => LABEL_KINDS.find((x) => x.kind === k) ?? LABEL_KINDS[LABEL_KINDS.length - 1]!;

/** How much to trust a number from a wrist sensor. */
export const GRADES: Record<'A' | 'B' | 'C' | 'D', string> = {
  A: 'Solid on wrist data',
  B: 'Good for your own trends',
  C: 'Experimental',
  D: 'Shown for completeness; wrist sensors measure this poorly',
};

/** The HRV4Training-style advice texts (verbatim from the HRV notebook) and their colors. */
export const ADVICE: Record<string, { label: string; text: string; color: string }> = {
  normal: { label: 'Normal', text: 'Within your usual range: train as planned.', color: '#2ee6c5' },
  high: { label: 'High', text: 'Higher than usual: you are recovered, a good day for a hard session.', color: '#7aa2ff' },
  watch: { label: 'Watch', text: 'Lower than usual for now: keep an eye on it and see how you feel.', color: '#ffb547' },
  easy: { label: 'Keep it easy', text: 'Lower than usual for a few nights: keep today easy.', color: '#ff9a5c' },
  back_off: { label: 'Back off', text: 'Well below your range: back off and prioritise recovery.', color: '#ff6b6b' },
};

/** The engine's advice label ("normal", "high", "watch", "keep it easy", "back off") as an ADVICE entry. */
export function adviceOf(label: string | null | undefined) {
  const k = (label ?? 'normal').toLowerCase().replace(/\s+/g, '_');
  const key = k === 'keep_it_easy' ? 'easy' : k;
  return ADVICE[key] ?? ADVICE.normal!;
}
