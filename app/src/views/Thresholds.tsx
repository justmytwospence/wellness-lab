import { useCallback, useMemo } from 'react';
import { line as d3line } from 'd3-shape';
import type { Table } from '@wl/schema';
import { PAL, isoDay, mix } from '@wl/views';
import { useJob } from '../lib/useJob';
import { useParam } from '../lib/router';
import { col, finite, txt } from '../lib/util';
import { Empty } from '../charts/small';
import { Plot, type PlotCtx } from '../charts/Plot';
import { TimeChart } from '../charts/TimeChart';
import { Legend, Missing, NeedsLargerScreen, Select, Split, useNarrow } from '../charts/lab';
import { Panel } from '../ui/Panel';
import { View } from '../ui/View';
import { Segmented } from '../ui/Segmented';
import { Fill, Stack } from '../ui/Fill';
import { quantile, sortedFinite } from '../charts/loess';
import { A1Note, DurabilityNote } from '../charts/notes';

const SPORTS: [string, string, string][] = [['run', 'Running', 'running'], ['ski', 'Skiing', 'skiing'], ['ride', 'Cycling', 'cycling']];
const NEEDS_RR = 'beat-to-beat intervals (RR) from activities recorded with HRV logging';
const MAX_ART = 0.05, MAX_REP = 0.3;
const OLD = '#3a4d78';
const abs = { position: 'absolute', inset: 0 } as const;

interface Curve { hb: number[]; a1: number[]; n: number[]; line: (number | null)[]; vt1: number | null; vt2: number | null }
interface Now { vt1: number | null; vt2: number | null; as_of: string | null; vt1_all: number | null; vt2_all: number | null; activities: number }
interface A1Result { windows: Table; acts: Table; curves: Record<string, Curve | undefined>; history: Table; now: Record<string, Now | undefined>; durability: Table }

export default function Thresholds() {
  const narrow = useNarrow();
  if (narrow) return <NeedsLargerScreen name="Thresholds" />;
  return <ThresholdsView />;
}

/** The toolbar, the optional stat strip and the notes around a Thresholds body. */
function Frame({ sport, strip, children }: { sport: string; strip?: React.ReactNode; children: React.ReactNode }) {
  const [, setSport] = useParam('sport', 'run');
  return (
    <View title="Thresholds" desc="Aerobic and anaerobic thresholds from DFA a1 during exercise." strip={strip}
      tools={<Segmented role="radiogroup" label="Sport" value={sport} options={SPORTS.map(([k, t]) => [k, t] as [string, string])} onChange={setSport} />}>
      <Split aside={<><A1Note /><DurabilityNote /></>}>{children}</Split>
    </View>
  );
}

function ThresholdsView() {
  const [sport] = useParam('sport', 'run');
  const { data, loading, error } = useJob({ kind: 'dfa' as const });
  const r = data as unknown as A1Result | null;
  const label = SPORTS.find((s) => s[0] === sport)?.[2] ?? 'running';
  if (error) return <Frame sport={sport}><div className="err" role="alert" style={{ padding: 20 }}>{error}</div></Frame>;
  if (!r) return <Frame sport={sport}><Empty>{loading ? 'Computing DFA a1 for every recording (this can take a minute)\u2026' : 'No data yet.'}</Empty></Frame>;
  if (r.acts.n === 0) return <Frame sport={sport}><div className="vgrid"><Panel title="DFA a1 against heart rate" method="dfa.a1" job={{ kind: 'dfa' }}><Missing needs={NEEDS_RR} /></Panel></div></Frame>;
  return <Body r={r} sport={sport} label={label} />;
}

function Body({ r, sport, label }: { r: A1Result; sport: string; label: string }) {
  const [rec, setRec] = useParam('rec', '');
  const job = { kind: 'dfa' };
  // the activities of this sport, newest first (the last 400 are offered)
  const acts = useMemo(() => {
    const a = col(r.acts, 'act'), sp = txt(r.acts, 'sport'), day = col(r.acts, 'day'), min = col(r.acts, 'minutes'), name = txt(r.acts, 'name');
    const out: { act: number; day: number; minutes: number; name: string }[] = [];
    sp.forEach((s, i) => s === sport && out.push({ act: a[i]!, day: day[i]!, minutes: min[i]!, name: name[i] ?? '' }));
    return out.sort((x, y) => y.day - x.day || y.act - x.act);
  }, [r.acts, sport]);
  const byAct = useMemo(() => new Map(acts.map((a) => [a.act, a])), [acts]);

  // fresh windows (first hour) that pass the artefact and device checks
  const cloud = useMemo(() => {
    const t = col(r.windows, 't_s'), hr = col(r.windows, 'hr'), a1 = col(r.windows, 'a1'), art = col(r.windows, 'art'), rep = col(r.windows, 'rep'), act = col(r.windows, 'act');
    const idx: number[] = [];
    for (let i = 0; i < t.length; i++) if (t[i]! <= 3720 && art[i]! <= MAX_ART && rep[i]! <= MAX_REP && finite(a1[i]) && byAct.has(act[i]!)) idx.push(i);
    if (!idx.length) return null;
    const maxDay = Math.max(...idx.map((i) => byAct.get(act[i]!)!.day));
    const step = Math.max(1, Math.ceil(idx.length / 12000));
    const pts = idx.filter((_, k) => k % step === 0).map((i) => ({ hr: hr[i]!, a1: a1[i]!, age: maxDay - byAct.get(act[i]!)!.day }));
    const hs = sortedFinite(idx.map((i) => hr[i]!));
    return { pts, maxAge: Math.max(1, ...pts.map((p) => p.age)), x0: quantile(hs, 0.005) - 3, x1: quantile(hs, 0.999) + 3, n: idx.length };
  }, [r.windows, byAct]);

  const curve = r.curves[sport];
  const now = r.now[sport];
  const hist = useMemo(() => {
    const sp = txt(r.history, 'sport'), day = col(r.history, 'day'), v1 = col(r.history, 'vt1'), v2 = col(r.history, 'vt2'), n = col(r.history, 'activities');
    const ix = sp.map((s, i) => (s === sport ? i : -1)).filter((i) => i >= 0);
    return { days: ix.map((i) => day[i]!), vt1: ix.map((i) => v1[i]!), vt2: ix.map((i) => v2[i]!), n: ix.map((i) => n[i]!) };
  }, [r.history, sport]);
  const histLines = useMemo(() => [{ y: hist.vt1, stroke: PAL.hrv, width: 1.6, points: true, spanGaps: true }, { y: hist.vt2, stroke: PAL.alert, width: 1.6, points: true, spanGaps: true }], [hist]);

  const recs = acts.slice(0, 400);
  const recId = recs.some((a) => String(a.act) === rec) ? Number(rec) : recs[0]?.act;
  const cur = recId === undefined ? null : byAct.get(recId) ?? null;

  const draw = useCallback((g: CanvasRenderingContext2D, { sx, sy }: PlotCtx) => {
    if (!cloud) return;
    const buckets = new Map<number, typeof cloud.pts>();
    for (const p of cloud.pts) {
      const b = Math.round(Math.sqrt(p.age / cloud.maxAge) * 40);
      (buckets.get(b) ?? buckets.set(b, []).get(b)!).push(p);
    }
    g.globalAlpha = 0.35;
    for (const [b, ps] of buckets) {
      g.fillStyle = mix(PAL.hrv, OLD, b / 40);
      g.beginPath();
      for (const p of ps) { const x = sx(p.hr), y = sy(Math.min(1.8, Math.max(0, p.a1))); g.moveTo(x + 1.8, y); g.arc(x, y, 1.8, 0, Math.PI * 2); }
      g.fill();
    }
    g.globalAlpha = 1;
  }, [cloud]);

  if (!cloud) {
    return <Frame sport={sport}><div className="vgrid"><Panel title="DFA a1 against heart rate" method="dfa.a1" job={job}><Missing needs={`${label} recordings with beat intervals`} /></Panel></div></Frame>;
  }
  const thr = [curve?.vt1, curve?.vt2].filter((v): v is number => finite(v));
  return (
    <Frame sport={sport} strip={now ? <Summary now={now} curve={curve} label={label} /> : undefined}>
    <div className="vgrid" style={{ gridTemplateColumns: 'minmax(0, 1.2fr) minmax(0, 1fr)', gridTemplateRows: 'minmax(0, 1fr) minmax(0, 1fr)' }}>
      <Panel title={`DFA a1 against heart rate, every fresh ${label} window`} method="dfa.a1" job={job}
        actions={<Legend items={[{ color: PAL.hrv, label: 'recent' }, { color: OLD, label: 'older' }, { color: PAL.hr, label: 'fitted line', line: true }, { color: PAL.alert, label: 'a1 0.75 and 0.5', line: true }]} />}
        sub={`Dots: 2-minute windows from first hours (${cloud.n.toLocaleString('en')}, up to 12,000 drawn), older ones fainter. White: median per 2 bpm; amber: the line through the transition zone`}>
        <Fill>{({ h }) => (
        <Plot height={h} x={{ domain: [cloud.x0, cloud.x1], label: 'Heart rate (bpm)' }} y={{ domain: [0, 1.8], label: 'DFA a1', ticks: 6 }} draw={draw} label={`DFA a1 against heart rate for ${label}`}>
          {({ sx, sy, w }) => {
            const med = curve ? curve.hb.map((h, i) => [h, curve.a1[i]!] as [number, number]) : [];
            const fit = curve ? curve.hb.map((h, i) => [h, curve.line[i]] as [number, number | null]).filter((p): p is [number, number] => finite(p[1])) : [];
            const path = (pts: [number, number][]) => d3line<[number, number]>().x((p) => sx(p[0])).y((p) => sy(p[1]))(pts) ?? '';
            return (
              <>
                {[[0.75, '0.75 aerobic'], [0.5, '0.5 anaerobic']].map(([v, t]) => (
                  <g key={String(t)}>
                    <line x1={0} x2={w} y1={sy(v as number)} y2={sy(v as number)} stroke={PAL.alert} strokeDasharray="5 4" />
                    <text x={w - 4} y={sy(v as number) - 4} textAnchor="end" fill={PAL.alert} fontSize={11}>{t}</text>
                  </g>
                ))}
                {med.length > 1 && <path d={path(med)} fill="none" style={{ stroke: 'var(--text)' }} strokeWidth={1.2} opacity={0.7} />}
                {med.map((p) => <circle key={p[0]} cx={sx(p[0])} cy={sy(p[1])} r={2} style={{ fill: 'var(--text)' }} opacity={0.7} />)}
                {fit.length > 1 && <path d={path(fit)} fill="none" stroke={PAL.hr} strokeWidth={3} />}
                {thr.map((v) => (
                  <g key={v}>
                    <line x1={sx(v)} x2={sx(v)} y1={0} y2={sy(0)} stroke={PAL.hr} strokeWidth={1.5} />
                    <text x={sx(v) + 4} y={sy(1.7) - 2} fill={PAL.hr} fontSize={12} fontWeight={700}>{v.toFixed(0)} bpm</text>
                  </g>
                ))}
              </>
            );
          }}
        </Plot>
        )}</Fill>
      </Panel>

      <Panel title="Thresholds over time" method="dfa.a1" job={job} sub="Each point: all fresh windows of the six weeks before it"
        actions={hist.days.length >= 2 && (hist.vt1.some(finite) || hist.vt2.some(finite)) ? <Legend items={[{ color: PAL.hrv, label: 'aerobic (a1 0.75)', line: true }, { color: PAL.alert, label: 'anaerobic (a1 0.5)', line: true }]} /> : undefined}>
        {hist.days.length < 2 ? <Missing needs={`at least 3 ${label} recordings in six weeks`} /> : !hist.vt1.some(finite) && !hist.vt2.some(finite) ? (
          <Empty>No threshold found: alpha-1 has to cross 0.75 or 0.5 within the six-week windows, which takes recordings that run from easy to hard effort.</Empty>
        ) : (
          <Fill>{({ h }) => (
            <TimeChart label={`Thresholds over time for ${label}`} days={hist.days} lines={histLines} height={h} yTitle="Heart rate (bpm)"
              readout={(i) => `six weeks to ${isoDay(hist.days[i]!)}  aerobic ${finite(hist.vt1[i]) ? hist.vt1[i]!.toFixed(0) : '-'} bpm  anaerobic ${finite(hist.vt2[i]) ? hist.vt2[i]!.toFixed(0) : '-'} bpm  (${hist.n[i]} recordings)`} />
          )}</Fill>
        )}
      </Panel>

      <Panel title="One recording, window by window" method="dfa.a1" job={job} sub="Faded points failed the artefact or device checks"
        actions={<Select label="Recording" value={String(recId ?? '')} options={recs.map((a) => [String(a.act), `${isoDay(a.day)} \u00b7 ${a.name} \u00b7 ${a.minutes.toFixed(0)} min`] as [string, string])} onChange={(v) => setRec(v)} />}>
        {cur ? <Recording r={r} act={cur.act} now={now} /> : <Missing needs={`${label} recordings with beat intervals`} />}
      </Panel>

      <Durability r={r} sport={sport} label={label} byAct={byAct} job={job} />
    </div>
    </Frame>
  );
}

function Summary({ now, curve, label }: { now: Now; curve: Curve | undefined; label: string }) {
  const f = (v: number | null | undefined) => (finite(v) ? `${v!.toFixed(0)} bpm` : 'not found');
  return (
    <div className="strip" data-testid="thresholds-now">
      <div><span className="l">Sport</span>{label[0]!.toUpperCase() + label.slice(1)}</div>
      <div><span className="l">aerobic threshold (a1 0.75)</span><b className="mono">{f(now.vt1)}</b></div>
      <div><span className="l">anaerobic (a1 0.5)</span><b className="mono">{f(now.vt2)}</b></div>
      {now.as_of && <div><span className="l">six weeks to</span>{now.as_of}</div>}
      <div style={{ flex: 1, color: 'var(--muted)', font: '400 12px var(--font)' }}>all fresh windows: {f(curve?.vt1)} and {f(curve?.vt2)}; {now.activities} recordings</div>
    </div>
  );
}

function Recording({ r, act, now }: { r: A1Result; act: number; now: Now | undefined }) {
  const w = useMemo(() => {
    const t = col(r.windows, 't_s'), hr = col(r.windows, 'hr'), a1 = col(r.windows, 'a1'), art = col(r.windows, 'art'), rep = col(r.windows, 'rep'), a = col(r.windows, 'act');
    const out: { m: number; hr: number; a1: number; art: number; rep: number; ok: boolean }[] = [];
    a.forEach((x, i) => x === act && out.push({ m: t[i]! / 60, hr: hr[i]!, a1: a1[i]!, art: art[i]!, rep: rep[i]!, ok: art[i]! <= MAX_ART && rep[i]! <= MAX_REP }));
    return out.sort((p, q) => p.m - q.m);
  }, [r.windows, act]);
  if (!w.length) return <Empty>No usable windows in this recording.</Empty>;
  const xmax = Math.max(...w.map((p) => p.m)) + 1;
  const hrs = w.map((p) => p.hr).filter(finite);
  const zones = [{ y0: 0, y1: 0.5, c: PAL.alert }, { y0: 0.5, y1: 0.75, c: PAL.hr }, { y0: 0.75, y1: 1.8, c: PAL.hrv }];
  const rules = [now?.vt1, now?.vt2].filter((v): v is number => finite(v));
  const m = { l: 54 };
  return (
    <>
      <Legend items={[{ color: PAL.hrv, label: 'below VT1' }, { color: PAL.hr, label: 'between' }, { color: PAL.alert, label: 'above VT2' }]} />
      <Stack weights={[0.6, 0.4]}>
      <Fill style={abs}>{({ h }) => (
      <Plot height={h} margin={m} x={{ domain: [0, xmax], noGrid: true }} y={{ domain: [0, 1.8], clamp: true, label: 'DFA a1', ticks: 6 }} label="DFA a1 window by window for one recording">
        {({ sx, sy }) => (
          <>
            {zones.map((z) => <rect key={z.y0} x={0} width={sx(xmax)} y={sy(z.y1)} height={sy(z.y0) - sy(z.y1)} fill={z.c} opacity={0.16} />)}
            <path d={d3line<(typeof w)[number]>().x((p) => sx(p.m)).y((p) => sy(p.a1))(w.filter((p) => finite(p.a1))) ?? ''} fill="none" style={{ stroke: 'var(--text)' }} strokeWidth={1.6} />
            {w.filter((p) => finite(p.a1)).map((p, i) => (
              <circle key={i} cx={sx(p.m)} cy={sy(p.a1)} r={2.6} style={{ fill: 'var(--text)' }} opacity={p.ok ? 1 : 0.25}>
                <title>{`${p.m.toFixed(1)} min: a1 ${p.a1.toFixed(2)}, ${p.hr.toFixed(0)} bpm, ${(p.art * 100).toFixed(1)} % corrected, ${(p.rep * 100).toFixed(0)} % repeats`}</title>
              </circle>
            ))}
          </>
        )}
      </Plot>
      )}</Fill>
      <Fill style={abs}>{({ h }) => (
      <Plot height={h} margin={m} x={{ domain: [0, xmax], label: 'Minutes into the recording' }} y={{ domain: [Math.min(...hrs) - 3, Math.max(...hrs) + 3], label: 'HR (bpm)' }} label="Heart rate for the same recording">
        {({ sx, sy, w: pw }) => (
          <>
            {rules.map((v) => <line key={v} x1={0} x2={pw} y1={sy(v)} y2={sy(v)} stroke={PAL.muted} strokeDasharray="4 4" />)}
            <path d={d3line<(typeof w)[number]>().x((p) => sx(p.m)).y((p) => sy(p.hr))(w.filter((p) => finite(p.hr))) ?? ''} fill="none" stroke={PAL.hr} strokeWidth={1.6} />
          </>
        )}
      </Plot>
      )}</Fill>
      </Stack>
    </>
  );
}

function Durability({ r, sport, label, byAct, job }: { r: A1Result; sport: string; label: string; byAct: Map<number, { name: string; day: number }>; job: unknown }) {
  const d = useMemo(() => {
    const act = col(r.durability, 'act'), block = col(r.durability, 'block'), resid = col(r.durability, 'resid'), sp = txt(r.durability, 'sport');
    const per = new Map<number, [number, number][]>();
    const blocks = new Map<number, number[]>();
    sp.forEach((s, i) => {
      if (s !== sport || !finite(resid[i])) return;
      (per.get(act[i]!) ?? per.set(act[i]!, []).get(act[i]!)!).push([block[i]!, resid[i]!]);
      (blocks.get(block[i]!) ?? blocks.set(block[i]!, []).get(block[i]!)!).push(resid[i]!);
    });
    const mean = [...blocks.entries()].filter(([, v]) => v.length >= 5).sort((a, b) => a[0] - b[0]).map(([b, v]) => [b, v.reduce((s, x) => s + x, 0) / v.length, v.length] as [number, number, number]);
    return { per: [...per.entries()].map(([a, p]) => ({ act: a, pts: p.sort((x, y) => x[0] - y[0]) })), mean };
  }, [r.durability, sport]);
  const sub = 'Grey: each activity of 2 h or more; amber: their mean (blocks with 5 or more activities)';
  if (!d.per.length) return <Panel title="a1 later in long activities" method="dfa.a1" job={job} sub={sub}><Empty>No long recordings for {label}: durability needs activities of two hours or more with beat intervals.</Empty></Panel>;
  const xmax = Math.max(1, ...d.per.flatMap((p) => p.pts.map((q) => q[0]))) + 0.25;
  return (
    <Panel title="a1 later in long activities" method="dfa.a1" job={job} sub={sub}>
      <Fill>{({ h }) => (
      <Plot height={h} x={{ domain: [0, xmax], label: 'Hours into the activity' }} y={{ domain: [-0.6, 0.6], clamp: true, label: 'a1 vs first hour, same HR' }} label="DFA a1 drift in long activities">
        {({ sx, sy, w }) => {
          const ln = d3line<[number, number]>().x((p) => sx(p[0])).y((p) => sy(p[1]));
          return (
            <>
              <line x1={0} x2={w} y1={sy(0)} y2={sy(0)} stroke={PAL.muted} strokeDasharray="3 3" />
              {d.per.map((p) => <path key={p.act} d={ln(p.pts) ?? ''} fill="none" stroke={PAL.muted} strokeWidth={0.8} opacity={0.3}><title>{`${byAct.get(p.act)?.name ?? ''} ${byAct.has(p.act) ? isoDay(byAct.get(p.act)!.day) : ''}`}</title></path>)}
              {d.mean.length > 1 && <path d={ln(d.mean.map((m) => [m[0], m[1]])) ?? ''} fill="none" stroke={PAL.hr} strokeWidth={3} />}
              {d.mean.map((m) => <circle key={m[0]} cx={sx(m[0])} cy={sy(m[1])} r={4} fill={PAL.hr}><title>{`${m[0]} h: ${m[1] >= 0 ? '+' : ''}${m[1].toFixed(2)} (${m[2]} activities)`}</title></circle>)}
            </>
          );
        }}
      </Plot>
      )}</Fill>
    </Panel>
  );
}
