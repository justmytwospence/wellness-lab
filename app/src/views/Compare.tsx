import { useMemo, useState } from 'react';
import { LABEL_KINDS, kindInfo, type Table } from '@wl/schema';
import { PAL, WEEKDAYS, formatSel, isoDay, parseDay, parseSel, signed, weekday } from '@wl/views';
import { useApp } from '../lib/app';
import { useJob } from '../lib/useJob';
import { navigate, useParam } from '../lib/router';
import { col } from '../lib/util';
import { DeltaBar, PairedStrip } from '../charts/analysis/dots';
import { Card, Desktop, Intro, JobNote } from '../charts/analysis/frame';
import { fmt, metricLabel, toRanges } from '../charts/analysis/inputs';

interface FeatureDiff { feature: string; median_sel: number | null; median_base: number | null; cliffs_delta: number | null; ks_d: number | null; n_sel: number; n_base: number }
interface Compared { features: FeatureDiff[]; labels: [string, number, number][]; n_sel: number; n_base: number }

const expand = (r: [number, number][]) => r.flatMap(([a, b]) => Array.from({ length: Math.max(0, b - a + 1) }, (_, i) => a + i));

function Row({ f, nights, sel, base }: { f: FeatureDiff; nights: Table; sel: Set<number>; base: Set<number> }) {
  const { a, b } = useMemo(() => {
    const d = col(nights, 'day'), v = col(nights, f.feature);
    const a: number[] = [], b: number[] = [];
    d.forEach((day, i) => { if (sel.has(day)) a.push(v[i]!); else if (base.has(day)) b.push(v[i]!); });
    return { a, b };
  }, [nights, f.feature, sel, base]);
  const delta = f.cliffs_delta ?? NaN;
  return (
    <tr data-testid="compare-row">
      <td>{metricLabel(f.feature)}</td>
      <td className="n">{fmt(f.median_sel)}</td>
      <td className="n">{fmt(f.median_base)}</td>
      <td><div className="row" style={{ gap: 6, flexWrap: 'nowrap' }}><DeltaBar v={delta} /><span className="mono" style={{ width: 44, textAlign: 'right' }}>{signed(delta, 2)}</span></div></td>
      <td className="n">{fmt(f.ks_d)}</td>
      <td className="n muted">{f.n_sel} / {f.n_base}</td>
      <td><PairedStrip sel={a} base={b} /></td>
    </tr>
  );
}

function Body() {
  const app = useApp();
  const { nights, state, range } = app;
  const [selP, setSel] = useParam('sel', '');
  const [bs, setBs] = useParam('bs', 'span');
  const [all, setAll] = useState(false);
  const [text, setText] = useState<string | null>(null);
  const sel = useMemo(() => parseSel(selP), [selP]);
  const selDays = useMemo(() => new Set(expand(sel)), [sel]);
  const days = useMemo(() => col(nights, 'day'), [nights]);
  const baseRanges = useMemo(
    () => (bs === 'range' ? toRanges(days.filter((d) => d >= range[0] && d <= range[1] && !selDays.has(d))) : undefined),
    [bs, days, range, selDays],
  );
  const baseDays = useMemo(() => {
    if (baseRanges) return new Set(expand(baseRanges));
    const lo = Math.min(...selDays), hi = Math.max(...selDays);
    return new Set(days.filter((d) => d >= lo && d <= hi && !selDays.has(d)));
  }, [baseRanges, days, selDays]);
  const job = useMemo(
    () => (nights && nights.n > 30 && sel.length ? { kind: 'compare' as const, sel, ...(baseRanges?.length ? { base: baseRanges } : {}) } : null),
    [nights, sel, baseRanges],
  );
  const { data, loading, error } = useJob(job);
  const res = data as unknown as Compared | null;
  const kinds = useMemo(() => {
    const m = new Map<string, number>();
    for (const l of state.labels) if (!l.deleted_at) m.set(l.kind, (m.get(l.kind) ?? 0) + 1);
    return m;
  }, [state.labels]);

  const pickKind = (k: string) => {
    if (!k) return;
    setSel(formatSel(toRanges(state.labels.filter((l) => !l.deleted_at && l.kind === k).flatMap((l) => expand([[l.start_day, l.end_day]])))));
  };
  const pickWeekday = (w: string) => {
    if (w === '') return;
    setSel(formatSel(days.filter((d) => d >= range[0] && d <= range[1] && weekday(d) === +w).map((d) => [d, d] as [number, number])));
  };
  const pickMonth = (m: string) => {
    if (!m) return;
    const a = parseDay(`${m}-01`);
    const [y, mo] = m.split('-').map(Number) as [number, number];
    const b = parseDay(`${mo === 12 ? y + 1 : y}-${String(mo === 12 ? 1 : mo + 1).padStart(2, '0')}-01`) - 1;
    setSel(formatSel([[a, b]]));
  };
  const rows = res ? (all ? res.features : res.features.slice(0, 25)) : [];
  const selText = sel.map(([a, b]) => (a === b ? isoDay(a) : `${isoDay(a)} to ${isoDay(b)}`)).join(', ');

  return (
    <div className="page">
      <h1>Compare</h1>
      <Intro>What is different about a chosen set of nights. Every night feature is ranked by Cliff's delta: the chance a chosen night is higher than a base night, minus the chance it is lower (+1 always higher, 0 no difference, -1 always lower).</Intro>
      <div className="card">
        <div className="row">
          <label>Label kind <select value="" onChange={(e) => pickKind(e.target.value)} aria-label="Select by label kind">
            <option value="">choose...</option>
            {LABEL_KINDS.filter((k) => kinds.get(k.kind)).map((k) => <option key={k.kind} value={k.kind}>{k.name} ({kinds.get(k.kind)})</option>)}
          </select></label>
          <label>Weekday <select value="" onChange={(e) => pickWeekday(e.target.value)} aria-label="Select by weekday">
            <option value="">choose...</option>
            {WEEKDAYS.map((w, i) => <option key={w} value={i}>{w}</option>)}
          </select></label>
          <label>Month <input type="month" onChange={(e) => pickMonth(e.target.value)} aria-label="Select by month" /></label>
          <label>Base <select value={bs} onChange={(e) => setBs(e.target.value)} aria-label="Base nights">
            <option value="span">Other nights within the selection's span</option>
            <option value="range">All other nights in the date range</option>
          </select></label>
        </div>
        <div className="row" style={{ marginTop: 8 }}>
          <label style={{ flex: 1 }}>Selection <input style={{ width: '100%' }} className="mono" value={text ?? selP} onChange={(e) => setText(e.target.value)} onBlur={() => { if (text !== null) { setSel(formatSel(parseSel(text)) || null); setText(null); } }} onKeyDown={(e) => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur(); }} placeholder="day ranges, a-b,c-d (from Nights: shift-drag)" aria-label="Selection (day ranges)" /></label>
          {selP && <button onClick={() => setSel(null)}>Clear</button>}
        </div>
        {sel.length > 0 && <p className="muted" style={{ fontSize: 12, margin: '6px 0 0' }}>{selText}</p>}
      </div>
      {!nights || nights.n === 0 ? <div className="empty">No nights loaded yet.</div>
        : sel.length === 0 ? <div className="empty" data-testid="no-selection">Choose nights above, or shift-drag nights in the Nights view and pick "Compare these nights".</div>
        : (
          <>
            <JobNote loading={loading} error={error} />
            {res && (
              <Card title={`Ranked differences: ${res.n_sel} chosen nights against ${res.n_base} base nights`} method="compare.nights" job={job} testid="compare-ranked">
                {res.n_sel < 3 || res.n_base < 3 ? <div className="empty">At least 3 nights are needed on each side.</div> : (
                  <>
                    <div className="row muted" style={{ fontSize: 12, marginBottom: 6 }}><span style={{ color: PAL.alert }}>&#9679; chosen</span><span style={{ color: PAL.base }}>&#9679; base</span><span>medians as ticks</span></div>
                    <table className="t">
                      <thead><tr><th>Feature</th><th className="n">Chosen median</th><th className="n">Base median</th><th>Cliff's delta</th><th className="n">KS D</th><th className="n">n</th><th>Nights</th></tr></thead>
                      <tbody>{rows.map((f) => <Row key={f.feature} f={f} nights={nights} sel={selDays} base={baseDays} />)}</tbody>
                    </table>
                    {res.features.length > 25 && <button onClick={() => setAll(!all)} style={{ marginTop: 8 }}>{all ? 'Show the top 25' : `Show all ${res.features.length}`}</button>}
                  </>
                )}
              </Card>
            )}
            {res && res.labels.length > 0 && (
              <Card title="Tags and labels: share of nights" method="compare.nights" job={job} testid="compare-labels">
                <table className="t">
                  <thead><tr><th>Label</th><th className="n">Chosen</th><th className="n">Base</th><th /></tr></thead>
                  <tbody>
                    {res.labels.map(([k, a, b]) => (
                      <tr key={k}>
                        <td><span style={{ color: kindInfo(k as never).color }}>&#9679;</span> {kindInfo(k as never).name}</td>
                        <td className="n">{Math.round(a * 100)}%</td>
                        <td className="n">{Math.round(b * 100)}%</td>
                        <td style={{ width: 200 }}>
                          <svg width={180} height={16} role="img" aria-label="share of nights">
                            <rect x={0} y={1} width={180 * a} height={6} fill={PAL.alert} />
                            <rect x={0} y={9} width={180 * b} height={6} fill={PAL.base} />
                          </svg>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </Card>
            )}
            <p className="muted" style={{ fontSize: 12 }}>Open a night: <a href={`/night/${sel[0]![0]}`} onClick={(e) => { e.preventDefault(); navigate(`/night/${sel[0]![0]}`); }}>{isoDay(sel[0]![0])}</a>.</p>
          </>
        )}
    </div>
  );
}

export default function Compare() {
  return <Desktop><Body /></Desktop>;
}
