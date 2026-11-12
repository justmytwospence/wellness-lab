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
import { METRICS, fmt, metricLabel, missingInput } from '../charts/analysis/inputs';

interface BenchRow {
  metric: string; grade: 'A' | 'B' | 'C' | 'D'; n_quiet: number; mean: number | null; sd_quiet: number | null; te: number | null; cv_pct: number | null; icc: number | null;
  split_half: number | null; q: number | null; sigma_eps: number | null; sigma_eta: number | null; mdc95: number | null; swc: number | null; sens_hard: number | null;
  sens: Record<string, [number | null, number | null, number | null, number]>; snr: number | null;
}

/** Nights with no event near them, an easy day before, no change of time zone around them and a full heart-rate record (as the engine's bench). */
function quietNights(nights: Table, spans: { a: number; b: number }[]): boolean[] {
  const d = col(nights, 'day'), tz = col(nights, 'tz_change'), cov = col(nights, 'hr_cov'), cls = txt(nights, 'load_class');
  return d.map((day, i) => {
    if (spans.some((s) => day >= s.a - 3 && day <= s.b + 3)) return false;
    if (cls[i] !== 'rest' && cls[i] !== 'easy') return false;
    for (let k = Math.max(0, i - 3); k <= Math.min(d.length - 1, i + 3); k++) if (tz[k] !== 0 && Number.isFinite(tz[k]!)) return false;
    return (cov[i] ?? 0) >= 0.8;
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
    return { x, lines, band };
  }, [nights, row, spec, state.labels]);
  return (
    <Card title={`${metricLabel(row.metric)}: quiet nights and the local level`} method="bench.reliability" job={{ kind: 'bench', metrics: [row.metric] }} testid="bench-detail">
      <LineChart x={chart.x} lines={chart.lines} band={chart.band} time xFmt={dayText} yLabel={spec?.log ? `ln ${row.metric}` : row.metric} height={220} ariaLabel={`${metricLabel(row.metric)}: nightly values and the fitted local level`} testid="chart-bench" />
      <p className="muted" style={{ fontSize: 12 }}>
        Orange dots are quiet nights (no event within 3 nights, an easy day before, no time-zone change), grey are the rest. The teal line is a random-walk level fitted by maximum likelihood (night-to-night noise {fmt(row.sigma_eps)}, drift {fmt(row.sigma_eta)} per night, drift to noise ratio q = {fmt(row.q)}), with +/- 1 SD of the level shaded.
        A change must exceed {fmt(row.mdc95)} to be real at 95% (the minimal detectable change); {fmt(row.swc)} is a worthwhile change (0.5 SD of quiet nights).
      </p>
    </Card>
  );
}

type Col = { id: string; label: string; title?: string; get(r: BenchRow): number | null; show(r: BenchRow): string };

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
    { id: 'n_quiet', label: 'n quiet', get: (r) => r.n_quiet, show: (r) => String(r.n_quiet) },
    { id: 'te', label: 'TE', title: 'Typical error: spread of consecutive quiet-night differences over root two', get: (r) => r.te, show: (r) => fmt(r.te) },
    { id: 'cv_pct', label: 'CV %', get: (r) => r.cv_pct, show: (r) => fmt(r.cv_pct, 1) },
    { id: 'icc', label: 'ICC', title: 'Reliability of consecutive quiet nights', get: (r) => r.icc, show: (r) => fmt(r.icc) },
    { id: 'split_half', label: 'Split-half', title: 'Odd against even five-minute values (HRV)', get: (r) => r.split_half, show: (r) => fmt(r.split_half) },
    { id: 'q', label: 'q', title: 'Level drift variance over noise variance', get: (r) => r.q, show: (r) => fmt(r.q) },
    { id: 'mdc95', label: 'MDC95', title: 'Minimal detectable change at 95%', get: (r) => r.mdc95, show: (r) => fmt(r.mdc95) },
    { id: 'swc', label: 'SWC', title: 'Smallest worthwhile change (0.5 SD of quiet nights)', get: (r) => r.swc, show: (r) => fmt(r.swc) },
    { id: 'sens_hard', label: 'Hard days', title: 'Rest-day minus hard-day deviation from the 7-night baseline, in SD', get: (r) => r.sens_hard, show: (r) => fmt(r.sens_hard) },
    ...kinds.map<Col>((k) => ({
      id: `sens:${k}`, label: kindInfo(k as never).name, title: `Mean response over the first three nights of ${k}, in SD, with its interval`,
      get: (r) => r.sens[k]?.[0] ?? null,
      show: (r) => { const s = r.sens[k]; return s && s[0] !== null ? `${fmt(s[0])} [${fmt(s[1])}, ${fmt(s[2])}]` : '\u2013'; },
    })),
    { id: 'snr', label: 'Signal to noise', title: "How many typical errors the metric's largest response is", get: (r) => r.snr, show: (r) => fmt(r.snr, 1) },
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
    <div className="page">
      <h1>Bench</h1>
      <Intro>The bench says which of your numbers carry information. For each metric it measures how much of the night-to-night movement is noise on quiet nights (typical error, reliability, minimal detectable change) and how strongly it responds to training and to the events you have confirmed. Metrics with a high signal to noise are worth watching; the rest mostly wobble.</Intro>
      <JobNote loading={loading} error={error} />
      {!nights || nights.n === 0 ? <div className="empty">No nights loaded yet.</div>
        : rows && rows.length === 0 ? <div className="empty" data-testid="bench-empty">Not enough quiet nights yet: each metric needs at least 20 nights with no event nearby, an easy day before and a full heart-rate record.</div>
        : rows && (
          <>
            <Card title="Every metric" method="bench.reliability" job={job} testid="bench-table">
              <div style={{ overflowX: 'auto' }}>
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
                        {cols.map((c) => <td key={c.id} className="n" style={{ whiteSpace: 'nowrap' }}>{c.show(r)}</td>)}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </Card>
            {sel && nights && <Detail row={sel} nights={nights} />}
            {skipped.length > 0 && (
              <details>
                <summary className="muted">{skipped.length} metrics not scored</summary>
                <ul>{skipped.map((m) => <li key={m.id}>{metricLabel(m.id)}: {missingInput(nights, m.id) ? <NotAvailable inline input={missingInput(nights, m.id)!} /> : 'fewer than 20 quiet nights with a value'}</li>)}</ul>
              </details>
            )}
          </>
        )}
    </div>
  );
}

export default function Bench() {
  return <Desktop><Body /></Desktop>;
}
