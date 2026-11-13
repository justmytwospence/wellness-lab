import { useMemo, useState } from 'react';
import { kindInfo, type Align, type RasterMeta } from '@wl/schema';
import { clock, formatSel, isoDay } from '@wl/views';
import { useApp } from '../lib/app';
import { useJob } from '../lib/useJob';
import { col } from '../lib/util';
import { navigate, useParam } from '../lib/router';
import { Raster } from '../charts/Raster';
import { Empty } from '../charts/small';
import { View, ViewEmpty } from '../ui/View';
import { Panel } from '../ui/Panel';
import { Fill } from '../ui/Fill';
import { Segmented } from '../ui/Segmented';
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
  const [bin, setBin] = useParam('bin', '2');
  const binMin = Math.max(1, Math.min(10, Number(bin) || 2));
  const [hover, setHover] = useState<{ row: number; col: number } | null>(null);
  useEffect(() => {
    const on = (e: KeyboardEvent) => { if (e.key === 'a' && !/^(INPUT|TEXTAREA|SELECT)$/.test((e.target as HTMLElement).tagName)) setAlign(align === 'onset' ? 'clock' : 'onset'); };
    window.addEventListener('keydown', on);
    return () => window.removeEventListener('keydown', on);
  }, [align, setAlign]);
  const { data, loading, error } = useJob(nights && nights.n ? { kind: 'raster', metric, align: align as Align, from: range[0], to: range[1], bin_min: binMin } : null);
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
    const rowBytes = meta.width * 4;
    const out = new Uint8Array(rgba.length);
    if (sort === 'date') out.set(rgba);
    else order.forEach((src, dst) => out.set(rgba.subarray(src * rowBytes, (src + 1) * rowBytes), dst * rowBytes));
    // fine bins are finer than the watch's own sampling: bridge gaps of up to two empty bins from the left
    if (meta.bin_min < 5) {
      for (let r = 0; r < meta.height; r++) {
        const o = r * rowBytes;
        for (let c = 1; c < meta.width - 1; c++) {
          const p = o + c * 4;
          if (out[p + 3] !== 0) continue;
          let k = c + 1;
          while (k < meta.width && k <= c + 2 && out[o + k * 4 + 3] === 0) k++;
          if (k > c + 2 || k >= meta.width || out[p - 1] === 0) continue;
          for (let j = c; j < k; j++) out.copyWithin(o + j * 4, p - 4, p);
          c = k - 1;
        }
      }
    }
    return out;
  }, [meta, rgba, order, sort]);
  if (!nights || !nights.n) return <ViewEmpty title="Nights">No data yet. Import or load the demo from the Data page.</ViewEmpty>;
  const labelAt = (day: number) => state.labels.filter((l) => !l.deleted_at && l.start_day <= day && l.end_day >= day);
  const hoverDay = hover && meta ? meta.days[order[hover.row] ?? hover.row] : undefined;
  const hoverMin = hover && meta ? hover.col * meta.bin_min : 0;
  const rasterJob = { kind: 'raster', metric, align, from: range[0], to: range[1], bin_min: binMin };
  return (
    <View
      title="Nights"
      desc="Every night as a row. Click a night to open it; shift-drag to compare."
      tools={(
        <>
          <label>Metric <select value={metric} onChange={(e) => setMetric(e.target.value)}>{METRICS.map((m) => <option key={m}>{m}</option>)}</select></label>
          <label>Align <select value={align} onChange={(e) => setAlign(e.target.value)}><option value="onset">on falling asleep</option><option value="clock">on the clock</option></select></label>
          <label>Sort <select value={sort} onChange={(e) => setSort(e.target.value)}>{SORTS.map((m) => <option key={m}>{m}</option>)}</select></label>
          <Segmented label="Bin size" value={String(binMin)} options={[['1', '1 min'], ['2', '2 min'], ['5', '5 min']]} onChange={setBin} />
          <MethodButton id="night.hr" job={rasterJob} />
        </>
      )}
    >
      <Panel style={{ padding: '10px 12px 8px' }} error={error} loading={!error && (!meta || !sorted) && loading}>
        {!meta || !sorted ? <Empty>Nothing to draw</Empty> : (
          <>
            <div style={{ display: 'flex', gap: 6, flex: '1 1 0', minHeight: 0 }}>
              <div style={{ width: 8, flex: 'none', position: 'relative' }}>
                {order.map((src, r) => {
                  const ls = labelAt(meta.days[src]!);
                  if (!ls[0]) return null;
                  return <div key={r} style={{ position: 'absolute', left: 0, right: 0, top: `${(r / meta.height) * 100}%`, height: `max(1px, ${100 / meta.height}%)`, background: kindInfo(ls[0].kind).color }} title={ls.map((l) => kindInfo(l.kind).name).join(', ')} />;
                })}
              </div>
              <Fill style={{ flex: '1 1 0' }}>
                {() => (
                  <Raster fit rgba={sorted} width={meta.width} height={meta.height}
                    onHover={(row, c) => setHover({ row, col: c })}
                    onClick={(row) => navigate(`/night/${meta.days[order[row] ?? row]}`)}
                    onSelect={(a, b) => { const ds = order.slice(a, b + 1).map((i) => meta.days[i]!).sort((x, y) => x - y); if (ds.length) navigate(`/compare?sel=${formatSel(ds.map((d) => [d, d] as [number, number]))}`); }} />
                )}
              </Fill>
            </div>
            <div className="mono muted" style={{ position: 'relative', height: 16, margin: '3px 0 0 14px', fontSize: 11 }}>
              {(() => {
                const span = align === 'onset' ? meta.span_min : 1440;
                const step = align === 'onset' ? 60 : 120;
                return Array.from({ length: Math.floor(span / step) + 1 }, (_, k) => k * step).map((m) => (
                  <span key={m} style={{ position: 'absolute', left: `${(m / span) * 100}%`, transform: m === 0 ? 'none' : m >= span ? 'translateX(-100%)' : 'translateX(-50%)' }}>
                    {align === 'onset' ? `${m / 60}h` : clock(m)}
                  </span>
                ));
              })()}
            </div>
            <div className="row muted" style={{ fontSize: 12, marginTop: 4, flexWrap: 'nowrap', justifyContent: 'space-between' }}>
              <span className="mono" style={{ color: hoverDay !== undefined ? 'var(--text)' : undefined }}>{hoverDay !== undefined ? `${isoDay(hoverDay)} · ${align === 'onset' ? `${(hoverMin / 60).toFixed(1)} h after falling asleep` : clock(hoverMin)}` : 'Hover a pixel for its night and time'}</span>
              <span>{meta.days.length} nights · {meta.bin_min}-minute bins · {meta.log ? 'log' : 'linear'} colour {meta.domain[0].toFixed(0)} to {meta.domain[2].toFixed(0)}</span>
            </div>
          </>
        )}
      </Panel>
    </View>
  );
}
