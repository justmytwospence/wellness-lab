import { useCallback, useMemo } from 'react';
import { area as d3area, line as d3line } from 'd3-shape';
import type { Table } from '@wl/schema';
import { PAL, isoDay } from '@wl/views';
import { useJob } from '../lib/useJob';
import { useNumParam, useParam } from '../lib/router';
import { col, finite, txt } from '../lib/util';
import { Empty } from '../charts/small';
import { Plot, type PlotCtx } from '../charts/Plot';
import { ChartCard, GOOD, Legend, Missing, NeedsLargerScreen, Select, Slider, Split, useNarrow, useNotes } from '../charts/lab';
import { quantile, sortedFinite } from '../charts/loess';
import { CleaningNote, PoincareNote, RrNote, WindowNote, WorkedNote } from '../charts/notes';

const NEEDS_RR = 'beat-to-beat intervals (RR) from activities recorded with HRV logging (at least 500 usable beats)';
const STATUS = ['kept', 'artifact', 'out of range'] as const;
const STATUS_COLORS = [PAL.hrv, PAL.alert, PAL.hr];

interface Detail {
  id: string; counts: Record<string, number>; beats: Table; window_minute: number; window: Table; window_rmssd: number; session_rmssd: number; mean_hr: number;
  poincare: { sd1: number; sd2: number; x: number[]; y: number[] };
  spectrum: { f: number[]; p: number[]; vlf: number; lf: number; hf: number; lf_hf: number };
}

export default function Beats() {
  const narrow = useNarrow();
  if (narrow) return <NeedsLargerScreen name="Beats" />;
  return <BeatsView />;
}

function BeatsView() {
  const notes = useNotes();
  const [sid, setSid] = useParam('s', '');
  const [minute, setMinute] = useNumParam('min', -1);
  const sessions = useJob({ kind: 'sessions' as const });
  const s = sessions.data as unknown as Table | null;
  const list = useMemo(() => {
    if (!s) return [];
    const id = txt(s, 'id'), name = txt(s, 'name'), q = txt(s, 'quality'), day = col(s, 'day'), beats = col(s, 'beats'), hr = col(s, 'avg_hr');
    // label format of the notebook: quality, date, name, beats, average heart rate (cleanest first)
    return id.map((x, i) => ({ id: x ?? '', label: `${q[i]} \u00b7 ${isoDay(day[i]!)} \u00b7 ${name[i] ?? ''} \u00b7 ${beats[i]!.toLocaleString('en')} beats \u00b7 avg ${finite(hr[i]) ? Math.trunc(hr[i]!) : '-'} bpm` }));
  }, [s]);
  const id = list.some((x) => x.id === sid) ? sid : list[0]?.id ?? '';
  const base = useJob(id ? { kind: 'beats' as const, id } : null);
  const win = useJob(id && minute >= 0 ? { kind: 'beats' as const, id, window_min: minute } : null);
  const d = ((minute >= 0 ? win.data : null) ?? base.data) as unknown as Detail | null;
  const err = sessions.error ?? base.error ?? win.error;

  const body = (() => {
    if (sessions.error) return <div className="card err" role="alert">{sessions.error}</div>;
    if (!s) return <Empty>{sessions.loading ? 'Looking for recordings with beat intervals...' : 'No data yet.'}</Empty>;
    if (s.n === 0) return <ChartCard title="RR tachogram" method="beats.cleaning" job={{ kind: 'sessions' }}><Missing needs={NEEDS_RR} /></ChartCard>;
    return (
      <>
        <div className="controls" style={{ margin: 0 }}>
          <Select label="Recording (cleanest first)" value={id} options={list.map((x) => [x.id, x.label] as [string, string])} onChange={(v) => { setSid(v); setMinute(null); }} />
        </div>
        {err ? <div className="card err" role="alert">{err}</div> : !d ? <Empty>Cleaning the beat stream...</Empty> : <Session d={d} minute={minute} setMinute={setMinute} />}
      </>
    );
  })();

  const aside = d ? <BeatNotes d={d} /> : <RrNote />;
  return (
    <div className="page">
      <h1>Beats</h1>
      <p className="muted">From the heartbeats of a chest-strap recording to RMSSD, the Poincare plot and the spectrum. Press <kbd>n</kbd> for the notes.</p>
      <Split notes={notes} aside={aside}>{body}</Split>
    </div>
  );
}

function keptRepeatShare(b: Table): number {
  const rr = col(b, 'rr'), st = col(b, 'status');
  let prev = NaN, same = 0, n = 0;
  rr.forEach((v, i) => {
    if (st[i] !== 0) return;
    if (Number.isFinite(prev)) { n++; if (v === prev) same++; }
    prev = v;
  });
  return n ? same / n : 0;
}

function windowStats(w: Table) {
  const d2 = col(w, 'd2').filter(finite);
  return { usable: d2.length, meanSq: d2.length ? d2.reduce((s, v) => s + v, 0) / d2.length : 0 };
}

function BeatNotes({ d }: { d: Detail }) {
  const rep = useMemo(() => keptRepeatShare(d.beats), [d.beats]);
  const ws = useMemo(() => windowStats(d.window), [d.window]);
  return (
    <>
      <RrNote />
      <CleaningNote counts={d.counts} total={d.beats.n} rep={rep} />
      <WindowNote />
      <WorkedNote usable={ws.usable} meanSq={ws.meanSq} winRmssd={d.window_rmssd} sessionRmssd={d.session_rmssd} hr={d.mean_hr} />
      <PoincareNote />
    </>
  );
}

function Session({ d, minute, setMinute }: { d: Detail; minute: number; setMinute(v: number | null): void }) {
  const jb = { kind: 'beats', id: d.id };
  const tmax = useMemo(() => { const t = col(d.beats, 't'); return t.length ? t[t.length - 1]! : 0; }, [d.beats]);
  const maxMin = Math.max(Math.floor(tmax / 60) - 1, 0);
  return (
    <>
      <Tachogram d={d} job={jb} />
      <ChartCard title="One minute, step by step" method="beats.rmssd" job={{ ...jb, window_min: d.window_minute }}
        subtitle="The window opens on the most variable minute of the recording; slide it anywhere.">
        <div className="controls" style={{ marginTop: 0 }}>
          <Slider label="Window start (minute)" min={0} max={maxMin} step={1} value={minute >= 0 ? minute : d.window_minute} onCommit={(v) => setMinute(v)} fmt={(v) => String(v)} />
        </div>
        <Steps d={d} />
      </ChartCard>
      <ChartCard title="Worked example" method="beats.rmssd" job={{ ...jb, window_min: d.window_minute }} subtitle="The first beats of the window, the way you would work them by hand">
        <Worked w={d.window} />
      </ChartCard>
      <div className="cols">
        <PoincarePlot d={d} job={jb} />
        <SpectrumPlot d={d} job={jb} />
      </div>
    </>
  );
}

// ---- Tachogram ------------------------------------------------------------------------------------------------

function Tachogram({ d, job }: { d: Detail; job: unknown }) {
  const g = useMemo(() => {
    const t = col(d.beats, 't'), rr = col(d.beats, 'rr'), st = col(d.beats, 'status'), beat = col(d.beats, 'beat');
    const k = sortedFinite(rr.filter((_, i) => st[i] === 0));
    return { t, rr, st, beat, lo: quantile(k, 0.002), hi: quantile(k, 0.998), tmax: t.length ? t[t.length - 1]! / 60 : 1 };
  }, [d.beats]);
  const y0 = Math.max(g.lo - 150, 250), y1 = g.hi + 250;
  const draw = useCallback((c: CanvasRenderingContext2D, { sx, sy }: PlotCtx) => {
    const clampY = (v: number) => sy(Math.min(y1, Math.max(y0, v)));
    for (const flagged of [false, true]) {
      for (let s = 0; s < 3; s++) {
        if ((s !== 0) !== flagged) continue;
        c.fillStyle = STATUS_COLORS[s]!;
        c.globalAlpha = flagged ? 1 : 0.35;
        const r = flagged ? 3.4 : 1.6;
        c.beginPath();
        for (let i = 0; i < g.t.length; i++) {
          if (g.st[i] !== s) continue;
          const x = sx(g.t[i]! / 60), y = clampY(g.rr[i]!);
          c.moveTo(x + r, y);
          c.arc(x, y, r, 0, Math.PI * 2);
        }
        c.fill();
      }
    }
    c.globalAlpha = 1;
  }, [g, y0, y1]);
  return (
    <ChartCard title="RR tachogram" method="beats.cleaning" job={job} subtitle="Every interval in the recording; flagged ones enlarged (pinned to the edge when off the scale)">
      <Legend items={STATUS.map((s, i) => ({ label: s, color: STATUS_COLORS[i]! }))} />
      <Plot height={270} x={{ domain: [0, g.tmax], label: 'Minutes into the recording' }} y={{ domain: [y0, y1], clamp: true, label: 'RR interval (ms)' }} draw={draw} label="RR tachogram colored by cleaning status" />
    </ChartCard>
  );
}

// ---- One-minute steps -----------------------------------------------------------------------------------------

function Steps({ d }: { d: Detail }) {
  const w = useMemo(() => {
    const sec = col(d.window, 'sec'), rr = col(d.window, 'rr'), st = col(d.window, 'status'), dd = col(d.window, 'd'), d2 = col(d.window, 'd2'), beat = col(d.window, 'beat');
    return sec.map((s, i) => ({ sec: s, rr: rr[i]!, st: st[i]!, d: dd[i]!, d2: d2[i]!, beat: beat[i]! }));
  }, [d.window]);
  const ok = w.filter((p) => finite(p.d));
  if (!w.length) return <Empty>No beats in this window.</Empty>;
  const { meanSq } = windowStats(d.window);
  const rrs = w.map((p) => p.rr).filter(finite);
  const dm = Math.max(5, ...ok.map((p) => Math.abs(p.d))) * 1.1;
  const d2m = Math.max(10, ...ok.map((p) => p.d2), meanSq) * 1.1;
  const x = { domain: [0, 60] as [number, number] };
  const heading = (t: string, s: string) => <div style={{ margin: '6px 0 0' }}><b style={{ fontSize: 13 }}>{t}</b> <span className="muted" style={{ fontSize: 12 }}>{s}</span></div>;
  return (
    <>
      {heading('1. RR intervals', 'Rising and falling with each breath (respiratory sinus arrhythmia)')}
      <Plot height={150} x={x} y={{ domain: [Math.min(...rrs) - 10, Math.max(...rrs) + 10], label: 'RR (ms)' }} label="RR intervals in the window">
        {({ sx, sy }) => (
          <>
            <path d={d3line<(typeof w)[number]>().x((p) => sx(p.sec)).y((p) => sy(p.rr))(w.filter((p) => finite(p.rr))) ?? ''} fill="none" stroke={PAL.hrv} strokeWidth={1.6} />
            {w.map((p) => <circle key={p.beat} cx={sx(p.sec)} cy={sy(p.rr)} r={3} fill={STATUS_COLORS[p.st] ?? PAL.hrv}><title>{`beat ${p.beat}: ${p.rr.toFixed(0)} ms, ${STATUS[p.st]}`}</title></circle>)}
          </>
        )}
      </Plot>
      {heading('2. Successive differences', 'Green: next beat slower; orange: next beat faster')}
      <Plot height={130} x={x} y={{ domain: [-dm, dm], label: 'diff (ms)' }} label="Successive differences in the window">
        {({ sx, sy }) => ok.map((p) => <rect key={p.beat} x={sx(p.sec) - 1.5} width={3} y={Math.min(sy(0), sy(p.d))} height={Math.abs(sy(p.d) - sy(0))} fill={p.d > 0 ? GOOD : PAL.hr}><title>{`beat ${p.beat}: ${p.d >= 0 ? '+' : ''}${p.d.toFixed(0)} ms`}</title></rect>)}
      </Plot>
      {heading('3. Squared differences', `Dashed line: their mean, ${Math.round(meanSq).toLocaleString('en')} ms\u00b2; its square root is RMSSD = ${finite(d.window_rmssd) ? d.window_rmssd.toFixed(1) : '-'} ms`)}
      <Plot height={130} x={{ ...x, label: 'Seconds into the window' }} y={{ domain: [0, d2m], label: 'Squared (ms\u00b2)' }} label="Squared differences in the window">
        {({ sx, sy, w: pw }) => (
          <>
            {ok.map((p) => <rect key={p.beat} x={sx(p.sec) - 1.5} width={3} y={sy(p.d2)} height={sy(0) - sy(p.d2)} fill={PAL.violet}><title>{`beat ${p.beat}: ${Math.round(p.d2).toLocaleString('en')} ms\u00b2`}</title></rect>)}
            <line x1={0} x2={pw} y1={sy(meanSq)} y2={sy(meanSq)} style={{ stroke: 'var(--text)' }} strokeDasharray="5 3" strokeWidth={1.5} />
          </>
        )}
      </Plot>
    </>
  );
}

function Worked({ w }: { w: Table }) {
  const rows = useMemo(() => {
    const beat = col(w, 'beat'), sec = col(w, 'sec'), rr = col(w, 'rr'), st = col(w, 'status'), d = col(w, 'd'), d2 = col(w, 'd2');
    return beat.slice(0, 12).map((b, i) => ({ b, sec: sec[i]!, rr: rr[i]!, st: STATUS[st[i]!] ?? '', d: d[i]!, d2: d2[i]! }));
  }, [w]);
  const f = (v: number, dg = 0) => (finite(v) ? v.toFixed(dg) : '');
  return (
    <table className="t" aria-label="Worked example">
      <thead><tr><th className="n">beat</th><th className="n">time (s)</th><th className="n">RR (ms)</th><th>status</th><th className="n">next minus this (ms)</th><th className="n">squared (ms&sup2;)</th></tr></thead>
      <tbody>{rows.map((r) => <tr key={r.b}><td className="n">{r.b}</td><td className="n">{f(r.sec, 2)}</td><td className="n">{f(r.rr)}</td><td>{r.st}</td><td className="n">{f(r.d)}</td><td className="n">{f(r.d2)}</td></tr>)}</tbody>
    </table>
  );
}

// ---- Poincare and spectrum ------------------------------------------------------------------------------------

function PoincarePlot({ d, job }: { d: Detail; job: unknown }) {
  const { x, y, sd1, sd2 } = d.poincare;
  const dom = useMemo(() => {
    const s = sortedFinite(x);
    return [quantile(s, 0.005), quantile(s, 0.995)] as [number, number];
  }, [x]);
  const draw = useCallback((c: CanvasRenderingContext2D, { sx, sy }: PlotCtx) => {
    c.fillStyle = PAL.hrv;
    c.globalAlpha = 0.35;
    c.beginPath();
    for (let i = 0; i < x.length; i++) { const px = sx(x[i]!), py = sy(y[i]!); c.moveTo(px + 2, py); c.arc(px, py, 2, 0, Math.PI * 2); }
    c.fill();
    c.globalAlpha = 1;
  }, [x, y]);
  if (!x.length || !(dom[1] > dom[0])) return <ChartCard title="Poincare plot" method="beats.rmssd" job={job}><Missing needs="at least two consecutive kept beats" /></ChartCard>;
  const mx = x.reduce((s, v) => s + v, 0) / x.length, my = y.reduce((s, v) => s + v, 0) / y.length;
  const k = Math.SQRT1_2;
  return (
    <ChartCard title="Poincare plot" method="beats.rmssd" job={job} subtitle={`SD1 ${sd1.toFixed(1)} ms (RMSSD / root 2), SD2 ${sd2.toFixed(1)} ms`}>
      <Plot height={320} x={{ domain: dom, label: 'RR(i) (ms)' }} y={{ domain: dom, label: 'RR(i+1) (ms)' }} draw={draw} label="Poincare plot of successive intervals">
        {({ sx, sy }) => (
          <>
            <line x1={sx(dom[0])} y1={sy(dom[0])} x2={sx(dom[1])} y2={sy(dom[1])} stroke={PAL.muted} strokeDasharray="4 3" />
            <line x1={sx(mx - sd2 * k)} y1={sy(my - sd2 * k)} x2={sx(mx + sd2 * k)} y2={sy(my + sd2 * k)} stroke={PAL.hr} strokeWidth={2.5} />
            <line x1={sx(mx + sd1 * k)} y1={sy(my - sd1 * k)} x2={sx(mx - sd1 * k)} y2={sy(my + sd1 * k)} stroke={PAL.violet} strokeWidth={2.5} />
            <text x={sx(mx + sd2 * k) + 4} y={sy(my + sd2 * k)} fill={PAL.hr} fontSize={12} fontWeight={700}>SD2</text>
            <text x={sx(mx - sd1 * k) - 4} y={sy(my + sd1 * k) - 4} textAnchor="end" fill={PAL.violet} fontSize={12} fontWeight={700}>SD1</text>
          </>
        )}
      </Plot>
    </ChartCard>
  );
}

const BANDS = [{ k: 'VLF', hi: 0.04, c: PAL.muted }, { k: 'LF', hi: 0.15, c: PAL.hr }, { k: 'HF', hi: 0.4, c: PAL.hrv }, { k: 'above HF', hi: Infinity, c: PAL.violet }];

function SpectrumPlot({ d, job }: { d: Detail; job: unknown }) {
  const { f, p, vlf, lf, hf, lf_hf } = d.spectrum;
  if (!f.length || !p.some((v) => v > 1e-4)) return <ChartCard title="Spectrum of the recording" method="beats.spectrum" job={job}><Missing needs="at least 5 minutes of beats (more than 100 kept beats per segment)" /></ChartCard>;
  const total = vlf + lf + hf || 1;
  const rows: [string, string, number][] = [['VLF', '0 to 0.04 Hz', vlf], ['LF', '0.04 to 0.15 Hz', lf], ['HF', '0.15 to 0.4 Hz', hf]];
  return (
    <ChartCard title="Spectrum of the recording" method="beats.spectrum" job={job} subtitle="Lomb-Scargle periodogram, averaged over half-overlapping 5-minute segments">
      <Legend items={BANDS.map((b) => ({ label: b.k, color: b.c }))} />
      <Plot height={250} x={{ domain: [0, 0.6], label: 'Frequency (Hz)' }} y={{ domain: [1e-4, 1], scale: 'log', label: 'Relative power (log)', ticks: [1e-4, 1e-3, 1e-2, 1e-1, 1].map((v) => ({ v, label: v === 1 ? '1' : v.toExponential(0).replace('e-', 'e\u2212') })) }} label="Spectrum with VLF, LF and HF bands">
        {({ sx, sy }) => {
          let lo = 0;
          return BANDS.map((b) => {
            const idx = f.map((v, i) => (v >= (lo ? BANDS[BANDS.indexOf(b) - 1]!.hi : 0) && v < b.hi ? i : -1)).filter((i) => i >= 0);
            lo = 1;
            if (idx.length < 2) return null;
            const pts = idx.map((i) => [f[i]!, p[i]!] as [number, number]);
            return <path key={b.k} d={d3area<[number, number]>().x((q) => sx(q[0])).y0(sy(1e-4)).y1((q) => sy(q[1]))(pts) ?? ''} fill={b.c} opacity={0.85} />;
          });
        }}
      </Plot>
      <table className="t" aria-label="Band power" style={{ marginTop: 8 }}>
        <thead><tr><th>Band</th><th>Range</th><th className="n">Power (ms&sup2;)</th><th className="n">Share</th></tr></thead>
        <tbody>
          {rows.map(([k, r, v]) => <tr key={k}><td>{k}</td><td className="muted">{r}</td><td className="n">{finite(v) ? v.toFixed(0) : ''}</td><td className="n">{finite(v) ? `${((v / total) * 100).toFixed(0)}%` : ''}</td></tr>)}
          <tr><td>LF / HF</td><td /><td className="n">{finite(lf_hf) ? lf_hf.toFixed(2) : ''}</td><td /></tr>
        </tbody>
      </table>
    </ChartCard>
  );
}
