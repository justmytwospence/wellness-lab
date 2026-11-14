import { useMemo } from 'react';
import type { Table } from '@wl/schema';
import { PAL, WEEKDAYS, clock, isoDay, weekday } from '@wl/views';
import { useApp } from '../lib/app';
import { useJob } from '../lib/useJob';
import { col, txt } from '../lib/util';
import { useParam } from '../lib/router';
import { Fill } from '../ui/Fill';
import { FocusList, type FocusItem } from '../ui/FocusList';
import { InspectorNotes } from '../ui/Inspector';
import { Panel } from '../ui/Panel';
import { View } from '../ui/View';
import { Empty } from '../charts/small';
import { ChartCard } from '../charts/wellness/ChartCard';
import { SeasonalView, type SeasonalSpec } from '../charts/wellness/seasonal';
import { TimeSeries, withAlpha, type TsSeries } from '../charts/wellness/ts';
import { BoxPlots, HeatGrid, Histogram, Legend, StackedAreas, StackedBars } from '../charts/wellness/svg';
import { SleepTiming, bedHour, rdylgn, type NightRow } from '../charts/wellness/trendcharts';
import { boxStats, densify, histogram, isNum, mean, monthOf, monthStartDay, quantile, rolling, yearOf } from '../charts/wellness/stats';

// The notebook's palette for categories (Training & Health: Recovery and Sleep)
const NB = { balanced: '#43aa8b', unbalanced: '#f8961e', low: '#f94144', none: '#9aa5b1', bb: '#277da1', resp: '#4d908e' };
const HRV_STATUS: [string, string, string][] = [['BALANCED', 'Balanced', NB.balanced], ['UNBALANCED', 'Unbalanced', NB.unbalanced], ['LOW', 'Low', NB.low], ['NONE', 'No status', NB.none]];
const READY_LEVELS: [string, string][] = [['PRIME', '#8d5a97'], ['HIGH', '#43aa8b'], ['MODERATE', '#90be6d'], ['LOW', '#f8961e'], ['POOR', '#f94144']];
const FACTORS: [string, string][] = [['tr_sleep_pct', 'Sleep'], ['tr_sleep_hist_pct', 'Sleep history'], ['tr_recovery_pct', 'Recovery time'], ['tr_hrv_pct', 'HRV'], ['tr_acwr_pct', 'Acute load'], ['tr_stress_hist_pct', 'Stress history']];
const STRESS_KEYS = [
  { key: 'rest', label: 'Rest', color: NB.bb },
  { key: 'low', label: 'Low', color: '#f9c74f' },
  { key: 'med', label: 'Medium', color: NB.unbalanced },
  { key: 'high', label: 'High', color: NB.low },
];
const STAGE_KEYS = [
  { key: 'deep', label: 'Deep', color: '#1d3557' },
  { key: 'rem', label: 'REM', color: '#8d5a97' },
  { key: 'light', label: 'Light', color: '#90b4ce' },
  { key: 'awake', label: 'Awake', color: NB.unbalanced },
];
const DAY_COLS = ['rhr', 'hrv_night', 'hrv_week', 'hrv_bal_lo', 'hrv_bal_hi', 'hrv_status', 'tr_score', 'tr_level', ...FACTORS.map((f) => f[0]), 'bb_charged', 'bb_high', 'bb_low', 'rest_s', 'low_s', 'med_s', 'high_s', 'sleep_score'];

/** The metrics offered to the box plots: the night column, how to show it, what it needs, and which method explains it. */
interface BoxMetric { id: string; label: string; input: string; method: string; fmt?: (v: number) => string; wrap?: boolean }
const hm = (v: number) => clock(v * 60);
const BOX_METRICS: BoxMetric[] = [
  { id: 'tst_h', label: 'Sleep time (h)', input: 'sleeps', method: 'night.timing' },
  { id: 'sleep_score', label: 'Garmin sleep score', input: 'days.sleep_score', method: 'garmin.teardown', fmt: (v) => v.toFixed(0) },
  { id: 'onset_clock', label: 'Fell asleep', input: 'sleeps', method: 'night.timing', fmt: hm, wrap: true },
  { id: 'wake_clock', label: 'Woke', input: 'sleeps', method: 'night.timing', fmt: hm },
  { id: 'mid_clock', label: 'Midsleep', input: 'sleeps', method: 'night.timing', fmt: hm },
  { id: 'eff', label: 'Sleep efficiency', input: 'sleeps', method: 'night.timing', fmt: (v) => v.toFixed(2) },
  { id: 'hr_min30', label: 'Lowest heart rate (bpm)', input: 'hr', method: 'night.hr', fmt: (v) => v.toFixed(0) },
  { id: 'dip_pct', label: 'Heart-rate dip (%)', input: 'hr', method: 'night.dip' },
  { id: 'rmssd_mean', label: 'Overnight HRV (ms)', input: 'hrv5', method: 'night.hrv', fmt: (v) => v.toFixed(0) },
  { id: 'resp_mean', label: 'Breathing rate (/min)', input: 'resp', method: 'night.resp' },
  { id: 'sleep_stress', label: 'Sleep stress', input: 'stress', method: 'night.stress', fmt: (v) => v.toFixed(0) },
  { id: 'spo2_mean', label: 'SpO2 asleep (%)', input: 'spo2', method: 'night.spo2' },
  { id: 'skin_temp', label: 'Skin temperature deviation', input: 'days.skin_temp_dev', method: 'garmin.teardown', fmt: (v) => v.toFixed(2) },
  { id: 'deep_min', label: 'Deep sleep (min)', input: 'stages', method: 'night.stages', fmt: (v) => v.toFixed(0) },
  { id: 'rem_min', label: 'REM sleep (min)', input: 'stages', method: 'night.stages', fmt: (v) => v.toFixed(0) },
  { id: 'bb_change', label: 'Body Battery charge', input: 'body_battery', method: 'garmin.teardown', fmt: (v) => v.toFixed(0) },
  { id: 'tr_score', label: 'Training readiness', input: 'days.tr_score', method: 'garmin.teardown', fmt: (v) => v.toFixed(0) },
];
const DAY_INPUT: Record<string, string> = {
  rhr: 'resting heart rate (days.rhr)', hrv_night: 'overnight HRV (days.hrv_night)', tr_score: 'training readiness (days.tr_score)',
  bb_high: 'Body Battery (days.bb_high)', rest_s: 'stress durations (days.rest_s)',
};
/** The metrics that get the By year and Radial views, by FocusItem id. */
const SEASONAL: Record<string, SeasonalSpec> = {
  rhr: { source: 'days', column: 'rhr', label: 'Resting heart rate', unit: 'bpm', digits: 1 },
  hrv: { source: 'days', column: 'hrv_night', label: 'Overnight HRV', unit: 'ms', digits: 0 },
  readiness: { source: 'days', column: 'tr_score', label: 'Training readiness', unit: 'points', digits: 0 },
  bb: { source: 'days', column: 'bb_high', label: 'Body Battery', unit: 'points', digits: 0 },
  spo2: { source: 'nights', column: 'spo2_mean', label: 'SpO2 asleep', unit: '%', digits: 1 },
  resp: { source: 'nights', column: 'resp_mean', label: 'Breathing rate asleep', unit: 'breaths/min', digits: 1 },
};
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

const allNaN = (v: ArrayLike<number | null>) => !Array.from(v as ArrayLike<number>).some(isNum);
const roll = (v: number[], w: number, min: number) => rolling(v, w, min);
const dotsAndMean = (label: string, v: number[], color: string, win = 7, min = 3): TsSeries[] => [
  { label, values: v, color: withAlpha(color, 0.55), mode: 'dots', size: 4 },
  { label: `${win}-day mean`, values: roll(v, win, min), color, width: 1.8 },
];

const DESC = 'Recovery and sleep, day by day.';
const lastFinite = (v: ArrayLike<number | null>) => { for (let i = v.length - 1; i >= 0; i--) if (isNum(v[i])) return v[i] as number; return NaN; };
const spark = (v: ArrayLike<number | null>) => Array.from(v as ArrayLike<number>).filter(isNum);

function TrendsView() {
  const app = useApp();
  const { nights, range } = app;
  const [from, to] = range;
  const [bm, setBm] = useParam('bm', 'tst_h');
  const have = !!nights && nights.n > 0;
  const dj = useJob(have ? { kind: 'days', from, to, columns: DAY_COLS } : null);
  const days = (dj.data ?? null) as Table | null;

  // daily tables on a dense day grid, so a missing day is a gap and the rolling means know it
  const D = useMemo(() => {
    if (!days) return null;
    const cols: Record<string, (number | null)[]> = {};
    for (const c of DAY_COLS) if (days.num[c]) cols[c] = days.num[c]!;
    const d = densify(col(days, 'day'), cols, from, to);
    const text = (name: string) => {
      const out = new Array<string | null>(d.x.length).fill(null);
      const dd = col(days, 'day');
      txt(days, name).forEach((s, i) => { const j = dd[i]! - from; if (j >= 0 && j < out.length) out[j] = s; });
      return out;
    };
    const get = (c: string) => d.cols[c] ?? new Array<number>(d.x.length).fill(NaN);
    return { x: d.x, get, hrvStatus: text('hrv_status'), trLevel: text('tr_level') };
  }, [days, from, to]);

  // the nights inside the range
  const N = useMemo(() => {
    if (!nights) return null;
    const day = col(nights, 'day');
    const idx = day.map((d, i) => (d >= from && d <= to ? i : -1)).filter((i) => i >= 0);
    const pick = (name: string) => { const c = col(nights, name); return idx.map((i) => c[i]!); };
    return { n: idx.length, pick, day: idx.map((i) => day[i]!) };
  }, [nights, from, to]);
  const dense = (name: string) => (N ? densify(N.day, { v: N.pick(name) }, from, to).cols.v! : []);

  if (!have) return <View title="Trends" desc={DESC}><Empty>No data yet. Import or load the demo from the Data page.</Empty></View>;
  if (!D || !N) return <View title="Trends" desc={DESC}><Empty>{dj.error ?? 'Working...'}</Empty></View>;

  const x = D.x;
  const dayNeed = (c: string) => (allNaN(D.get(c)) ? DAY_INPUT[c] ?? `days.${c}` : null);
  const rangeSub = `${isoDay(from)} to ${isoDay(to)}`;

  // 1. resting heart rate
  const rhr = D.get('rhr');
  // 2. Garmin nightly HRV and its balanced band
  const statusOf = (s: string | null) => (s === 'BALANCED' || s === 'UNBALANCED' || s === 'LOW' ? s : 'NONE');
  const hrvSeries: TsSeries[] = [
    { label: 'Balanced low', values: D.get('hrv_bal_lo'), color: NB.balanced, mode: 'none' },
    { label: 'Balanced high', values: D.get('hrv_bal_hi'), color: NB.balanced, mode: 'none' },
    ...HRV_STATUS.map(([k, label, color]): TsSeries => ({ label, values: D.get('hrv_night').map((v, i) => (statusOf(D.hrvStatus[i] ?? null) === k ? v : NaN)), color, mode: 'dots', size: 5 })),
    { label: '7-day average', values: D.get('hrv_week'), color: PAL.text, width: 1.6 },
  ];
  // 3. readiness
  const tr = D.get('tr_score');
  const readySeries: TsSeries[] = [
    ...READY_LEVELS.map(([k, color]): TsSeries => ({ label: k[0] + k.slice(1).toLowerCase(), values: tr.map((v, i) => (D.trLevel[i] === k ? v : NaN)), color, mode: 'dots', size: 5 })),
    { label: '14-day mean', values: roll(tr, 14, 5), color: PAL.text, width: 1.8 },
  ];
  // weekly factor heatmap
  const weeks: { start: number; v: number[] }[] = [];
  for (let ws = from - weekday(from); ws <= to; ws += 7) {
    const idx = x.map((d, i) => (d >= ws && d < ws + 7 ? i : -1)).filter((i) => i >= 0);
    weeks.push({ start: ws, v: FACTORS.map(([c]) => mean(idx.map((i) => D.get(c)[i]!))) });
  }
  const factorMissing = FACTORS.every(([c]) => allNaN(D.get(c)));
  // 4. Body Battery
  const bbLo = roll(D.get('bb_low'), 7, 3), bbHi = roll(D.get('bb_high'), 7, 3);
  // 5. stress hours per day by month
  const stressRows = (() => {
    const out: { day: number; values: number[] }[] = [];
    const m0 = monthOf(from), m1 = monthOf(to);
    for (let m = m0; m <= m1; m++) {
      const lo = monthStartDay(m), hi = monthStartDay(m + 1) - 1;
      const idx = x.map((d, i) => (d >= lo && d <= hi ? i : -1)).filter((i) => i >= 0);
      const v = ['rest_s', 'low_s', 'med_s', 'high_s'].map((c) => mean(idx.map((i) => D.get(c)[i]!)) / 3600);
      if (v.every(Number.isFinite)) out.push({ day: lo, values: v });
    }
    return out;
  })();
  // 6. SpO2 and breathing (nights)
  const spo2 = dense('spo2_mean'), resp = dense('resp_mean');
  // 7. every night
  const bedCol = N.pick('onset_clock'), wakeCol = N.pick('wake_clock'), scoreCol = N.pick('sleep_score'), tstCol = N.pick('tst_h');
  const timing: NightRow[] = N.day.flatMap((d, i) => {
    const bed = bedHour(bedCol[i]!), wake = wakeCol[i]!;
    return Number.isFinite(bed) && Number.isFinite(wake) && wake > bed && bed > -4 && wake < 11 ? [{ day: d, bed, wake, score: scoreCol[i]!, hours: tstCol[i]! }] : [];
  });
  // 8. monthly stage areas
  const stageRows = (() => {
    const cols = ['deep_min', 'light_min', 'rem_min', 'awake_min'].map((c) => N.pick(c));
    const byMonth = new Map<number, number[]>();
    N.day.forEach((d, i) => { if (Number.isFinite(cols[0]![i]!)) (byMonth.get(monthOf(d)) ?? byMonth.set(monthOf(d), []).get(monthOf(d))!).push(i); });
    return [...byMonth.entries()].filter(([, idx]) => idx.length >= 10).sort((a, b) => a[0] - b[0])
      .map(([m, idx]) => ({ day: monthStartDay(m) + 14, values: [0, 2, 1, 3].map((k) => mean(idx.map((i) => cols[k]![i]!)) / 60) }));
  })();
  // 9. box plots over every night on record
  const bmDef = BOX_METRICS.find((b) => b.id === bm) ?? BOX_METRICS[0]!;
  const allDay = col(nights, 'day');
  const allVal = col(nights, bmDef.id).map((v) => (bmDef.wrap && Number.isFinite(v) ? bedHour(v) : v));
  const group = (key: (d: number) => number, labels: [number, string][]) => labels.map(([k, label]) => ({ label, box: boxStats(allVal.filter((_, i) => key(allDay[i]!) === k)) }));
  const years = [...new Set(allDay.map(yearOf))].sort();
  const boxFmt = bmDef.fmt ? (bmDef.wrap ? (v: number) => clock(((v % 24) + 24) * 60) : bmDef.fmt) : (v: number) => (Math.abs(v) >= 100 ? v.toFixed(0) : v.toFixed(1));
  const boxMissing = allNaN(allVal) ? bmDef.input : null;
  // 10. sleep score histogram
  const score = N.pick('sleep_score').filter(isNum);
  const scoreMissing = score.length === 0 ? 'sleep score (days.sleep_score)' : null;

  const need = {
    rhr: dayNeed('rhr'), hrv: dayNeed('hrv_night'), readiness: dayNeed('tr_score'),
    drivers: factorMissing ? 'readiness factors (days.tr_sleep_pct and the other tr_*_pct columns)' : null,
    bb: dayNeed('bb_high'), stress: dayNeed('rest_s'),
    spo2: allNaN(spo2) ? 'spo2' : null, resp: allNaN(resp) ? 'resp' : null,
    timing: timing.length === 0 ? 'sleeps' : null, stages: allNaN(N.pick('deep_min')) ? 'stages' : null,
    score: scoreMissing, periods: boxMissing,
  };
  const val = (v: ArrayLike<number | null>, d: number): Pick<FocusItem, 'value' | 'spark'> => {
    const l = lastFinite(v);
    return { value: Number.isFinite(l) ? l.toFixed(d) : '-', spark: spark(v) };
  };
  const items: FocusItem[] = [
    { group: 'Recovery', id: 'rhr', title: 'Resting heart rate', na: need.rhr, ...val(rhr, 0) },
    { group: 'Recovery', id: 'hrv', title: 'Overnight HRV', na: need.hrv, ...val(D.get('hrv_night'), 0) },
    { group: 'Recovery', id: 'readiness', title: 'Training readiness', na: need.readiness, ...val(tr, 0) },
    { group: 'Recovery', id: 'drivers', title: 'What drives readiness', na: need.drivers },
    { group: 'Recovery', id: 'bb', title: 'Body Battery', na: need.bb, ...val(D.get('bb_high'), 0) },
    { group: 'Recovery', id: 'stress', title: 'Stress by month', na: need.stress },
    { group: 'Recovery', id: 'spo2', title: 'SpO2 asleep', na: need.spo2, ...val(spo2, 1) },
    { group: 'Recovery', id: 'resp', title: 'Breathing rate asleep', na: need.resp, ...val(resp, 1) },
    { group: 'Sleep', id: 'timing', title: 'Every night', na: need.timing },
    { group: 'Sleep', id: 'stages', title: 'Sleep stages by month', na: need.stages },
    { group: 'Sleep', id: 'score', title: 'Sleep score', na: need.score, ...val(N.pick('sleep_score'), 0) },
    { group: 'Sleep', id: 'periods', title: 'Sleep by weekday, month and year', na: need.periods },
  ];

  const render = (it: FocusItem) => {
    switch (it.id) {
      case 'rhr': return (
        <SeasonalView spec={SEASONAL.rhr!} from={from} to={to}>{(s) => (
          <ChartCard title="Resting heart rate" sub={s.sub ?? 'Daily value (dots) and 7-day mean (line)'} method={s.method ?? 'night.hr'} job={s.job ?? { kind: 'days', from, to, columns: ['rhr'] }} needs={need.rhr} actions={s.toggle}>
            <Fill>{({ h }) => s.seasonal ? s.seasonal(h) : <TimeSeries x={x} series={dotsAndMean('Resting HR', rhr, PAL.alert)} height={h} yLabel="bpm" sync="rec" yFmt={(v) => v.toFixed(0)} />}</Fill>
          </ChartCard>
        )}</SeasonalView>
      );
      case 'hrv': return (
        <SeasonalView spec={SEASONAL.hrv!} from={from} to={to}>{(s) => (
          <ChartCard title="Overnight HRV" sub={s.sub ?? "Garmin's nightly average by its status; line: Garmin's 7-day average; band: its balanced range"} method={s.method ?? 'baseline.normal_range'} job={s.job ?? { kind: 'days', from, to, columns: ['hrv_night', 'hrv_week', 'hrv_bal_lo', 'hrv_bal_hi', 'hrv_status'] }} needs={need.hrv} actions={s.toggle}>
            <Fill>{({ h }) => s.seasonal ? s.seasonal(h) : <TimeSeries x={x} series={hrvSeries} bands={[{ lo: 0, hi: 1, fill: withAlpha(NB.balanced, 0.18) }]} height={h} yLabel="ms" sync="rec" yFmt={(v) => v.toFixed(0)} />}</Fill>
            {!s.seasonal && <Legend items={HRV_STATUS.map(([, label, color]) => ({ label, color }))} />}
          </ChartCard>
        )}</SeasonalView>
      );
      case 'readiness': return (
        <SeasonalView spec={SEASONAL.readiness!} from={from} to={to}>{(s) => (
          <ChartCard title="Training readiness" sub={s.sub ?? "Each morning's score by Garmin's level; line: 14-day mean"} method={s.method ?? 'garmin.teardown'} job={s.job ?? { kind: 'days', from, to, columns: ['tr_score', 'tr_level'] }} needs={need.readiness} actions={s.toggle}>
            <Fill>{({ h }) => s.seasonal ? s.seasonal(h) : <TimeSeries x={x} series={readySeries} height={h} yLabel="score" sync="rec" yRange={[0, 100]} yFmt={(v) => v.toFixed(0)} />}</Fill>
            {!s.seasonal && <Legend items={READY_LEVELS.map(([k, color]) => ({ label: k[0] + k.slice(1).toLowerCase(), color }))} />}
          </ChartCard>
        )}</SeasonalView>
      );
      case 'drivers': return (
        <Panel title="What drives readiness" sub="Weekly average of each factor Garmin reports (100 = helping most)" method="garmin.teardown" job={{ kind: 'days', from, to, columns: FACTORS.map((f) => f[0]) }} needs={need.drivers} scroll>
          <HeatGrid
            rows={FACTORS.map((f) => f[1])}
            cols={weeks.length}
            color={(r, c) => { const v = weeks[c]!.v[r]!; return Number.isFinite(v) ? rdylgn(v / 100) : PAL.panel2; }}
            tip={(r, c) => `${FACTORS[r]![1]}, week of ${isoDay(weeks[c]!.start)}: ${Number.isFinite(weeks[c]!.v[r]!) ? weeks[c]!.v[r]!.toFixed(0) : 'no data'}`}
            xLabel={(c) => isoDay(weeks[c]!.start).slice(2, 7)}
            colLabelEvery={Math.max(1, Math.ceil(weeks.length / 12))}
          />
        </Panel>
      );
      case 'bb': return (
        <SeasonalView spec={SEASONAL.bb!} from={from} to={to}>{(s) => (
          <ChartCard title="Body Battery" sub={s.sub ?? 'Band: 7-day mean of the daily low to high; line: 7-day mean of the charge'} method={s.method ?? 'garmin.teardown'} job={s.job ?? { kind: 'days', from, to, columns: ['bb_low', 'bb_high', 'bb_charged'] }} needs={need.bb} actions={s.toggle}>
            <Fill>{({ h }) => s.seasonal ? s.seasonal(h) : (
              <TimeSeries x={x} series={[
                { label: 'Low', values: bbLo, color: NB.bb, mode: 'none' },
                { label: 'High', values: bbHi, color: NB.bb, mode: 'none' },
                { label: 'Charged', values: roll(D.get('bb_charged'), 7, 3), color: '#1d3557', width: 1.6 },
              ]} bands={[{ lo: 0, hi: 1, fill: withAlpha(NB.bb, 0.35) }]} height={h} yRange={[0, 100]} yFmt={(v) => v.toFixed(0)} />
            )}</Fill>
          </ChartCard>
        )}</SeasonalView>
      );
      case 'stress': return (
        <ChartCard title="Stress by month" sub="Average hours per day at each stress level" method="night.stress" job={{ kind: 'days', from, to, columns: ['rest_s', 'low_s', 'med_s', 'high_s'] }} needs={need.stress}>
          {stressRows.length ? <Fill>{({ h }) => <StackedBars rows={stressRows} keys={STRESS_KEYS} height={h} unit=" h" />}</Fill> : <Empty>No full months of stress data in this range.</Empty>}
          <Legend items={STRESS_KEYS.map((k) => ({ label: k.label, color: k.color }))} />
        </ChartCard>
      );
      case 'spo2': return (
        <SeasonalView spec={SEASONAL.spo2!} from={from} to={to}>{(s) => (
          <ChartCard title="SpO2 asleep" sub={s.sub ?? 'Nightly mean (dots) and 7-day mean'} method={s.method ?? 'night.spo2'} job={s.job ?? { kind: 'nights', from, to }} needs={need.spo2} actions={s.toggle}>
            <Fill>{({ h }) => s.seasonal ? s.seasonal(h) : <TimeSeries x={x} series={dotsAndMean('SpO2 %', spo2, NB.bb)} height={h} sync="br" yFmt={(v) => v.toFixed(1)} />}</Fill>
          </ChartCard>
        )}</SeasonalView>
      );
      case 'resp': return (
        <SeasonalView spec={SEASONAL.resp!} from={from} to={to}>{(s) => (
          <ChartCard title="Breathing rate asleep" sub={s.sub ?? 'Nightly mean (dots) and 7-day mean'} method={s.method ?? 'night.resp'} job={s.job ?? { kind: 'nights', from, to }} needs={need.resp} actions={s.toggle}>
            <Fill>{({ h }) => s.seasonal ? s.seasonal(h) : <TimeSeries x={x} series={dotsAndMean('Breaths/min', resp, NB.resp)} height={h} sync="br" yFmt={(v) => v.toFixed(1)} />}</Fill>
          </ChartCard>
        )}</SeasonalView>
      );
      case 'timing': return (
        <ChartCard title="Every night" sub="Each line runs from falling asleep to waking, colored by Garmin's sleep score" method="night.timing" job={{ kind: 'nights', from, to }} needs={need.timing}>
          {/* the score gradient under the chart takes one line */}
          <Fill>{({ h }) => <SleepTiming rows={timing} d0={from} d1={to} height={Math.max(80, h - 24)} />}</Fill>
        </ChartCard>
      );
      case 'stages': return (
        <ChartCard title="Sleep stages by month" sub="Monthly average per night; months with at least 10 nights" method="night.stages" job={{ kind: 'nights', from, to }} needs={need.stages}>
          {stageRows.length ? <Fill>{({ h }) => <StackedAreas rows={stageRows} keys={STAGE_KEYS} height={h} fmt={(v) => `${v.toFixed(1)}`} />}</Fill> : <Empty>Fewer than 10 nights with stages in any month of this range.</Empty>}
          <Legend items={STAGE_KEYS.map((k) => ({ label: k.label, color: k.color }))} />
        </ChartCard>
      );
      case 'score': return (
        <ChartCard title="Sleep score" sub={scoreMissing ? undefined : `Nights per 5-point bin; median ${quantile(score, 0.5).toFixed(0)}`} method="garmin.teardown" job={{ kind: 'days', from, to, columns: ['sleep_score'] }} needs={need.score}>
          <Fill>{({ h }) => <Histogram bins={histogram(score, 5)} color={NB.resp} height={h} xLabel="Sleep score" median={quantile(score, 0.5)} />}</Fill>
        </ChartCard>
      );
      default: return (
        <ChartCard
          title="Sleep by weekday, month and year"
          sub="Every night on record, not only the date range. Weekday is the night the sleep began on. Box: middle half; line: median; whiskers: 1.5 IQR."
          method={bmDef.method}
          job={{ kind: 'nights' }}
          needs={need.periods}
          actions={<label>Metric <select value={bmDef.id} onChange={(e) => setBm(e.target.value)} aria-label="Box plot metric">{BOX_METRICS.map((b) => <option key={b.id} value={b.id}>{b.label}</option>)}</select></label>}
        >
          <div className="vgrid" style={{ gridTemplateColumns: 'repeat(3, minmax(0,1fr))', height: 'auto', flex: '1 1 0' }}>
            {([
              ['Night of the week', group((d) => weekday(d - 1), WEEKDAYS.map((w, i) => [i, w])), undefined],
              ['Month', group((d) => new Date(d * 86_400_000).getUTCMonth(), MONTHS.map((m, i) => [i, m])), PAL.base],
              ['Year', group(yearOf, years.map((y) => [y, String(y)])), PAL.violet],
            ] as const).map(([title, groups, color]) => (
              <div key={title} className="panel" style={{ padding: '0 12px' }}>
                <h3>{title}</h3>
                <Fill>{({ h }) => <BoxPlots groups={groups} color={color} fmt={boxFmt} height={h} />}</Fill>
              </div>
            ))}
          </div>
        </ChartCard>
      );
    }
  };

  return (
    <View title="Trends" desc={DESC} tools={<span className="muted mono" style={{ fontSize: 12 }}>{rangeSub}; {N.n} nights</span>}>
      <InspectorNotes><h2>About this view</h2><p>Recovery and sleep over the date range, daily, with rolling means. Garmin's own scores are shown as Garmin computed them.</p></InspectorNotes>
      <FocusList items={items} render={render} />
    </View>
  );
}

export default function Trends() {
  return <TrendsView />;
}
