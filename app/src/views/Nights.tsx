import { useMemo, useState } from 'react';
import { kindInfo, type Align, type RasterMeta } from '@wl/schema';
import { clock, formatSel, isoDay } from '@wl/views';
import { useApp } from '../lib/app';
import { useJob } from '../lib/useJob';
import { col } from '../lib/util';
import { navigate, useNumParam, useParam } from '../lib/router';
import { Raster } from '../charts/Raster';
import { Empty } from '../charts/small';
import { MethodButton } from '../layout/MethodPanel';
import { useEffect } from 'react';

const METRICS = ['hr', 'hrv5', 'stress', 'resp', 'spo2', 'stage', 'steps', 'motion', 'altitude'];
const SORTS = ['date', 'hr_min30', 'ln_rmssd', 'tst_h', 'sleep_score', 'resp_mean', 'sleep_stress', 'onset_clock', 'mid_clock'];

export default function Nights() {
  const app = useApp();
  const { range, nights, state } = app;
  const [metric, setMetric] = useParam('metric', 'hr');
  const [align, setAlign] = useParam('align', 'onset');
  const [sort, setSort] = useParam('sort', 'date');
  const [rowPx, setRowPx] = useNumParam('row', 3);
  const [hover, setHover] = useState<{ row: number; col: number } | null>(null);
  useEffect(() => {
    const on = (e: KeyboardEvent) => { if (e.key === 'a' && !/^(INPUT|TEXTAREA|SELECT)$/.test((e.target as HTMLElement).tagName)) setAlign(align === 'onset' ? 'clock' : 'onset'); };
    window.addEventListener('keydown', on);
    return () => window.removeEventListener('keydown', on);
  }, [align, setAlign]);
  const { data, loading, error } = useJob(nights && nights.n ? { kind: 'raster', metric, align: align as Align, from: range[0], to: range[1] } : null);
  const meta = data as unknown as RasterMeta | null;
  const rgba = data?.arrays.rgba as Uint8Array | undefined;
  // rows in the requested order
  const order = useMemo(() => {
    if (!meta) return [] as number[];
    const idx = meta.days.map((_, i) => i);
    if (sort === 'date') return idx;
    const nd = col(nights, 'day'), nv = col(nights, sort);
    const by = new Map(nd.map((d, i) => [d, nv[i]!]));
    return idx.sort((a, b) => (by.get(meta.days[b]!) ?? -Infinity) - (by.get(meta.days[a]!) ?? -Infinity));
  }, [meta, sort, nights]);
  const sorted = useMemo(() => {
    if (!meta || !rgba) return null;
    if (sort === 'date') return rgba;
    const rowBytes = meta.width * 4;
    const out = new Uint8Array(rgba.length);
    order.forEach((src, dst) => out.set(rgba.subarray(src * rowBytes, (src + 1) * rowBytes), dst * rowBytes));
    return out;
  }, [meta, rgba, order, sort]);
  if (!nights || !nights.n) return <div className="page"><h1>Nights</h1><Empty>No data yet. Import or load the demo from the Data page.</Empty></div>;
  const labelAt = (day: number) => state.labels.filter((l) => !l.deleted_at && l.start_day <= day && l.end_day >= day);
  const hoverDay = hover && meta ? meta.days[order[hover.row] ?? hover.row] : undefined;
  const hoverMin = hover && meta ? hover.col * meta.bin_min : 0;
  return (
    <div className="page">
      <div className="row" style={{ justifyContent: 'space-between' }}>
        <h1>Nights</h1>
        <div className="row">
          <label>Metric <select value={metric} onChange={(e) => setMetric(e.target.value)}>{METRICS.map((m) => <option key={m}>{m}</option>)}</select></label>
          <label>Align <select value={align} onChange={(e) => setAlign(e.target.value)}><option value="onset">on falling asleep</option><option value="clock">on the clock</option></select></label>
          <label>Sort <select value={sort} onChange={(e) => setSort(e.target.value)}>{SORTS.map((m) => <option key={m}>{m}</option>)}</select></label>
          <label>Row height <input type="range" min={1} max={8} value={rowPx} onChange={(e) => setRowPx(+e.target.value)} /></label>
          <MethodButton id="night.hr" job={{ kind: 'raster', metric, align, from: range[0], to: range[1] }} />
        </div>
      </div>
      {error ? <p className="err">{error}</p> : !meta || !sorted ? <Empty>{loading ? 'Drawing...' : 'Nothing to draw'}</Empty> : (
        <div className="card" style={{ display: 'flex', gap: 6 }}>
          <div style={{ width: 8, flex: 'none' }}>
            {order.map((src, r) => { const ls = labelAt(meta.days[src]!); return <div key={r} style={{ height: rowPx, background: ls[0] ? kindInfo(ls[0].kind).color : 'transparent' }} title={ls.map((l) => kindInfo(l.kind).name).join(', ')} />; })}
          </div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <Raster rgba={sorted} width={meta.width} height={meta.height} rowPx={rowPx}
              onHover={(row, c) => setHover({ row, col: c })}
              onClick={(row) => navigate(`/night/${meta.days[order[row] ?? row]}`)}
              onSelect={(a, b) => { const ds = order.slice(a, b + 1).map((i) => meta.days[i]!).sort((x, y) => x - y); if (ds.length) navigate(`/compare?sel=${formatSel(ds.map((d) => [d, d] as [number, number]))}`); }} />
            <div className="row muted mono" style={{ fontSize: 12, justifyContent: 'space-between', marginTop: 4 }}>
              <span>{align === 'onset' ? '0 min' : '00:00'}</span>
              <span>{hoverDay !== undefined ? `${isoDay(hoverDay)}, ${align === 'onset' ? `${hoverMin} min after falling asleep` : clock(hoverMin)}` : 'Hover a pixel; click a night to open it; shift-drag to compare nights'}</span>
              <span>{align === 'onset' ? `${meta.span_min} min` : '24:00'}</span>
            </div>
            <p className="muted" style={{ fontSize: 12 }}>{meta.days.length} nights, {meta.bin_min}-minute bins. Colour: {meta.log ? 'log scale' : 'linear'} from {meta.domain[0].toFixed(0)} to {meta.domain[2].toFixed(0)}.</p>
          </div>
        </div>
      )}
    </div>
  );
}
