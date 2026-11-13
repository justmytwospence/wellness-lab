import { useMemo } from 'react';
import { kindInfo, type Table } from '@wl/schema';
import { PAL, isoDay } from '@wl/views';
import { useApp } from '../lib/app';
import { useJob } from '../lib/useJob';
import { useParam } from '../lib/router';
import { col, txt } from '../lib/util';
import { LineChart, type BandSpec, type LineSpec } from '../charts/analysis/lines';
import { Grade } from '../charts/small';
import { Card, Desktop, Intro, JobNote, NotAvailable } from '../charts/analysis/frame';
import { InspectorNotes } from '../ui/Inspector';
import { Fill } from '../ui/Fill';
import { View } from '../ui/View';
import { METRICS, fmt, metricLabel, missingInput } from '../charts/analysis/inputs';

interface BenchRow {
  metric: string; grade: 'A' | 'B' | 'C' | 'D'; n_quiet: number; mean: number | null; sd_quiet: number | null; te: number | null; cv_pct: number | null; icc: number | null;
  split_half: number | null; q: number | null; sigma_eps: number | null; sigma_eta: number | null; mdc95: number | null; swc: number | null; sens_hard: number | null;
  sens: Record<string, [number | null, number | null, number | null, number]>; snr: number | null;
}

/** Nights with no event near them, an easy day before, no change of time zone around them and a near-full heart-rate record (as the engine's bench). */
function quietNights(nights: Table, spans: { a: number; b: number }[]): boolean[] {
  const d = col(nights, 'day'), tz = col(nights, 'tz_change'), cov = col(nights, 'hr_cov'), cls = txt(nights, 'load_class');
  return d.map((day, i) => {
    if (spans.some((s) => day >= s.a - 3 && day <= s.b + 3)) return false;
    if (cls[i] !== 'rest' && cls[i] !== 'easy') return false;
    for (let k = Math.max(0, i - 3); k <= Math.min(d.length - 1, i + 3); k++) if (tz[k] !== 0 && Number.isFinite(tz[k]!)) return false;
    return (cov[i] ?? 0) >= 0.6;
  });
}

/** The local-level filter the engine fits: a random-walk level seen through noise. */
function localLevel(y: number[], se: number, sn: number): { level: number[]; sd: number[] } {
  const fin = y.filter(Number.isFinite);
  const m0 = fin.length ? fin.reduce((a, b) => a + b, 0) / fin.length : 0;
  const v0 = fin.length > 1 ? fin.reduce((a, b) => a + (b - m0) ** 2, 0) / (fin.length - 1) : 1;
  let mu = fin[0] ?? 0, p = 1e3 * Math.max(v0, 1e-12);
  const level: number[] = [], sd: number[] = [];
  for (const v of y) {
    const pp = p + sn * sn;
    if (Number.isFinite(v)) { const k = pp / (pp + se * se); mu += k * (v - mu); p = (1 - k) * pp; } else p = pp;
    level.push(mu); sd.push(Math.sqrt(p));
  }
  return { level, sd };
}

const dayText = (sec: number) => isoDay(Math.round(sec / 86400));

function Detail({ row, nights }: { row: BenchRow; nights: Table }) {
  const { state } = useApp();
  const spec = METRICS.find((m) => m.id === row.metric);
  const chart = useMemo(() => {
    const days = col(nights, 'day');
    const raw = col(nights, row.metric).map((v) => (spec?.log ? Math.log(v) : v));
    const quiet = quietNights(nights, state.labels.filter((l) => !l.deleted_at).map((l) => ({ a: l.start_day, b: l.end_day })));
    const { level, sd } = localLevel(raw, row.sigma_eps ?? 0, row.sigma_eta ?? 0);
    const x = days.map((d) => d * 86400);
    const lines: LineSpec[] = [
      { label: 'other nights', y: raw.map((v, i) => (quiet[i] ? NaN : v)), color: PAL.muted, width: 0, points: true, faint: true },
      { label: 'quiet nights', y: raw.map((v, i) => (quiet[i] ? v : NaN)), color: PAL.hr, width: 0, points: true },
      { label: 'level', y: level, color: PAL.hrv, width: 2 },
    ];
    const band: BandSpec = { lo: level.map((l, i) => l - sd[i]!), hi: level.map((l, i) => l + sd[i]!), color: PAL.hrv };
    // start at the metric's first value: years before it began are only the filter's prior
    const s0 = Math.max(0, raw.findIndex((v) => Number.isFinite(v)));
    const cut = <T,>(a: T[]) => a.slice(s0);
    return { x: cut(x), lines: lines.map((l) => ({ ...l, y: cut(l.y) })), band: { ...band, lo: cut(band.lo), hi: cut(band.hi) } };
  }, [nights, row, spec, state.labels]);
  return (
    <Card title={`${metricLabel(row.metric)}: quiet nights and the local level`} method="bench.reliability" job={{ kind: 'bench', metrics: [row.metric] }} testid="bench-detail">
      <Fill>{({ h }) => <LineChart x={chart.x} lines={chart.lines} band={chart.band} time xFmt={dayText} yLabel={spec?.log ? `ln ${row.metric}` : row.metric} height={Math.max(120, h)} ariaLabel={`${metricLabel(row.metric)}: nightly values and the fitted local level`} testid="chart-bench" />}</Fill>
      <InspectorNotes>
        <p className="muted">
          Orange dots are quiet nights (no event within 3 nights, an easy day before, no time-zone change), grey are the rest. The teal line is a random-walk level fitted by maximum likelihood (night-to-night noise {fmt(row.sigma_eps)}, drift {fmt(row.sigma_eta)} per night, drift to noise ratio q = {fmt(row.q)}), with +/- 1 SD of the level shaded.
          A change must exceed {fmt(row.mdc95)} to be real at 95% (the minimal detectable change); {fmt(row.swc)} is a worthwhile change (0.5 SD of quiet nights).
        </p>
      </InspectorNotes>
    </Card>
  );
}

type Col = { id: string; label: string; title?: string; get(r: BenchRow): number | null; show(r: BenchRow): string; tone?(r: BenchRow): string | undefined };

const clamp01 = (x: number) => Math.min(1, Math.max(0, x));
/** A cell tint for a score in 0..1: coral below the middle, teal above, clear in between. */
function goodness(t: number | null | undefined): string | undefined {
  if (t === null || t === undefined || !Number.isFinite(t)) return undefined;
  const d = clamp01(t) - 0.5;
  if (Math.abs(d) < 0.08) return undefined;
  return `color-mix(in srgb, var(${d > 0 ? '--hrv' : '--alert'}) ${Math.round(Math.abs(d) * 2 * 34)}%, transparent)`;
}
/** A response in SD: violet tint by size, faint when its interval crosses zero. */
function effect(v: number | null | undefined, lo?: number | null, hi?: number | null): string | undefined {
  if (v === null || v === undefined || !Number.isFinite(v)) return undefined;
  const sure = lo === undefined || lo === null || hi === null || hi === undefined || lo > 0 || hi < 0;
  const pct = Math.round(clamp01(Math.abs(v) / 1.5) * (sure ? 40 : 12));
  return pct < 4 ? undefined : `color-mix(in srgb, var(--violet) ${pct}%, transparent)`;
}

function Body() {
  const { nights } = useApp();
  const job = useMemo(() => (nights && nights.n > 30 ? { kind: 'bench' as const } : null), [nights]);
  const { data, loading, error } = useJob(job);
  const rows = (data as unknown as BenchRow[] | null) ?? null;
  const [sort, setSort] = useParam('sort', 'snr');
  const [dir, setDir] = useParam('dir', 'desc');
  const [pick, setPick] = useParam('metric', '');
  const kinds = useMemo(() => [...new Set((rows ?? []).flatMap((r) => Object.keys(r.sens)))].sort(), [rows]);
  const cols = useMemo<Col[]>(() => [
    { id: 'snr', label: 'Signal to noise', title: "How many typical errors the metric's largest response is", get: (r) => r.snr, show: (r) => fmt(r.snr, 1), tone: (r) => goodness(r.snr === null ? null : 0.5 + r.snr / 6) },
    { id: 'n_quiet', label: 'n quiet', get: (r) => r.n_quiet, show: (r) => String(r.n_quiet) },
    { id: 'te', label: 'TE', title: 'Typical error: spread of consecutive quiet-night differences over root two', get: (r) => r.te, show: (r) => fmt(r.te) },
    { id: 'cv_pct', label: 'CV %', get: (r) => r.cv_pct, show: (r) => fmt(r.cv_pct, 1), tone: (r) => goodness(r.cv_pct === null ? null : 1 - (r.cv_pct - 2) / 18) },
    { id: 'icc', label: 'ICC', title: 'Reliability of consecutive quiet nights', get: (r) => r.icc, show: (r) => fmt(r.icc), tone: (r) => goodness(r.icc === null ? null : (r.icc - 0.3) / 0.6) },
    { id: 'split_half', label: 'Split-half', title: 'Odd against even five-minute values (HRV)', get: (r) => r.split_half, show: (r) => fmt(r.split_half), tone: (r) => goodness(r.split_half === null ? null : (r.split_half - 0.3) / 0.6) },
    { id: 'q', label: 'q', title: 'Level drift variance over noise variance', get: (r) => r.q, show: (r) => fmt(r.q) },
    { id: 'mdc95', label: 'MDC95', title: 'Minimal detectable change at 95%', get: (r) => r.mdc95, show: (r) => fmt(r.mdc95) },
    { id: 'swc', label: 'SWC', title: 'Smallest worthwhile change (0.5 SD of quiet nights)', get: (r) => r.swc, show: (r) => fmt(r.swc) },
    { id: 'sens_hard', label: 'Hard days', title: 'Rest-day minus hard-day deviation from the 7-night baseline, in SD', get: (r) => r.sens_hard, show: (r) => fmt(r.sens_hard), tone: (r) => effect(r.sens_hard) },
    ...kinds.map<Col>((k) => ({
      id: `sens:${k}`, label: kindInfo(k as never).name, title: `Mean response over the first three nights of ${k}, in SD, with its interval`,
      get: (r) => r.sens[k]?.[0] ?? null,
      show: (r) => { const s = r.sens[k]; return s && s[0] !== null ? `${fmt(s[0])} [${fmt(s[1])}, ${fmt(s[2])}]` : '\u2013'; },
      tone: (r) => { const s = r.sens[k]; return s ? effect(s[0], s[1], s[2]) : undefined; },
    })),
  ], [kinds]);
  const sorted = useMemo(() => {
    if (!rows) return [];
    const c = cols.find((x) => x.id === sort);
    const key = (r: BenchRow) => (sort === 'metric' ? 0 : c ? (c.id.startsWith('sens:') ? Math.abs(c.get(r) ?? NaN) : c.get(r) ?? NaN) : r.snr ?? NaN);
    const s = [...rows].sort((a, b) => {
      if (sort === 'metric') return metricLabel(a.metric).localeCompare(metricLabel(b.metric));
      const [x, y] = [key(a), key(b)];
      if (!Number.isFinite(x)) return Number.isFinite(y) ? 1 : 0;
      if (!Number.isFinite(y)) return -1;
      return y - x;
    });
    return dir === 'asc' ? s.reverse() : s;
  }, [rows, cols, sort, dir]);
  const click = (id: string) => { if (id === sort) setDir(dir === 'desc' ? 'asc' : 'desc'); else { setSort(id); setDir('desc'); } };
  const arrow = (id: string) => (id === sort ? (dir === 'desc' ? ' \u25BE' : ' \u25B4') : '');
  const sel = (rows ?? []).find((r) => r.metric === pick) ?? sorted[0];
  const skipped = METRICS.filter((m) => rows && !rows.some((r) => r.metric === m.id));

  return (
    <View title="Bench" desc="Which of your numbers carry signal, and which mostly wobble." tools={<JobNote loading={loading} error={error} />}>
      <Intro>The bench says which of your numbers carry information. For each metric it measures how much of the night-to-night movement is noise on quiet nights (typical error, reliability, minimal detectable change) and how strongly it responds to training and to the events you have confirmed. Metrics with a high signal to noise are worth watching; the rest mostly wobble.</Intro>
      {!nights || nights.n === 0 ? <div className="empty">No nights loaded yet.</div>
        : rows && rows.length === 0 ? <div className="empty" data-testid="bench-empty">Not enough quiet nights yet: each metric needs at least 20 nights with no event nearby, an easy day before and a near-full heart-rate record.</div>
        : rows ? (
          <div className="vgrid" style={{ gridTemplateRows: 'minmax(0,1.3fr) minmax(0,1fr)' }}>
            <Card title="Every metric" method="bench.reliability" job={job} testid="bench-table" scroll>
              <table className="t" data-testid="bench-sortable">
                <thead>
                  <tr>
                    <th><button style={{ all: 'unset', cursor: 'pointer' }} onClick={() => click('metric')}>Metric{arrow('metric')}</button></th>
                    {cols.map((c) => <th key={c.id} className="n" title={c.title}><button style={{ all: 'unset', cursor: 'pointer' }} onClick={() => click(c.id)} aria-sort={c.id === sort ? (dir === 'desc' ? 'descending' : 'ascending') : 'none'}>{c.label}{arrow(c.id)}</button></th>)}
                  </tr>
                </thead>
                <tbody>
                  {sorted.map((r) => (
                    <tr key={r.metric} onClick={() => setPick(r.metric)} style={{ cursor: 'pointer', background: sel?.metric === r.metric ? 'var(--panel2)' : undefined }} aria-selected={sel?.metric === r.metric} data-testid="bench-row">
                      <td style={{ whiteSpace: 'nowrap' }}>{metricLabel(r.metric)} <Grade g={r.grade} /></td>
                      {cols.map((c) => <td key={c.id} className="n" style={{ whiteSpace: 'nowrap', background: c.tone?.(r), fontWeight: c.id === 'snr' ? 600 : undefined }}>{c.show(r)}</td>)}
                    </tr>
                  ))}
                </tbody>
              </table>
              <div className="legend" style={{ marginTop: 8 }}>
                <span><i style={{ background: 'color-mix(in srgb, var(--hrv) 40%, transparent)' }} />reliable or strong signal</span>
                <span><i style={{ background: 'color-mix(in srgb, var(--alert) 40%, transparent)' }} />noisy</span>
                <span><i style={{ background: 'color-mix(in srgb, var(--violet) 45%, transparent)' }} />response to training or an event, in SD (faint: interval crosses zero)</span>
              </div>
              {skipped.length > 0 && (
                <details style={{ marginTop: 10 }}>
                  <summary className="muted">{skipped.length} metrics not scored</summary>
                  <ul>{skipped.map((m) => <li key={m.id}>{metricLabel(m.id)}: {missingInput(nights, m.id) ? <NotAvailable inline input={missingInput(nights, m.id)!} /> : 'fewer than 20 quiet nights with a value'}</li>)}</ul>
                </details>
              )}
            </Card>
            {sel && nights ? <Detail row={sel} nights={nights} /> : <div className="panel" />}
          </div>
        ) : <div className="empty">{loading ? 'Working...' : ''}</div>}
    </View>
  );
}

export default function Bench() {
  return <Desktop><Body /></Desktop>;
}
