import { useEffect, useMemo, useRef, useState } from 'react';
import { PAL, isoDay } from '@wl/views';
import { AX } from './svg';

export interface ActogramProps {
  /** engine-drawn colors, `rows x width` pixels */
  rgba: Uint8Array;
  /** one byte per pixel: 1 where the person was asleep */
  asleep?: Uint8Array;
  width: number;
  height: number;
  /** the first day of each row */
  days: number[];
  periodH: number;
  overlay: boolean;
  /** fit the plot into this many CSS pixels of height (fractional row heights) */
  fitHeight?: number;
}

const GUTTER = 62;
const SLEEP = [122, 162, 255] as const;

/** A double-plotted actogram: each row shows two consecutive periods, so a drifting rhythm reads as a diagonal. Sleep is tinted over the activity. */
export function Actogram({ rgba, asleep, width, height, days, periodH, overlay, fitHeight }: ActogramProps) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [hover, setHover] = useState<{ r: number; c: number } | null>(null);
  const rowPx = fitHeight ? Math.max(0.25, (fitHeight - 36) / Math.max(1, height)) : Math.max(1, Math.min(4, Math.floor(1400 / Math.max(1, height))));
  const pixels = useMemo(() => {
    const out = new Uint8ClampedArray(rgba.length);
    out.set(rgba);
    if (overlay && asleep && asleep.length >= width * height) {
      for (let i = 0; i < width * height; i++) {
        if (!asleep[i]) continue;
        const o = i * 4;
        const empty = out[o + 3]! === 0;
        const a = empty ? 0.28 : 0.4;
        const base = empty ? [11, 16, 32] : [out[o]!, out[o + 1]!, out[o + 2]!];
        out[o] = base[0]! * (1 - a) + SLEEP[0] * a;
        out[o + 1] = base[1]! * (1 - a) + SLEEP[1] * a;
        out[o + 2] = base[2]! * (1 - a) + SLEEP[2] * a;
        out[o + 3] = 255;
      }
    }
    return out;
  }, [rgba, asleep, width, height, overlay]);
  useEffect(() => {
    const c = canvas.current!;
    c.width = width;
    c.height = height;
    c.getContext('2d')!.putImageData(new ImageData(pixels, width, height), 0, 0);
  }, [pixels, width, height]);

  const per = Math.round(periodH * 60);
  const at = (e: React.MouseEvent) => {
    const r = canvas.current!.getBoundingClientRect();
    return { r: Math.min(height - 1, Math.max(0, Math.floor(((e.clientY - r.top) / r.height) * height))), c: Math.min(width - 1, Math.max(0, Math.floor(((e.clientX - r.left) / r.width) * width))) };
  };
  const tip = (() => {
    if (!hover || !days.length) return '';
    const abs = hover.r * per + hover.c;
    const day = days[0]! + Math.floor(abs / 1440);
    const min = abs % 1440;
    const z = asleep && asleep[hover.r * width + hover.c] ? ', asleep' : '';
    return `${isoDay(day)} ${String(Math.floor(min / 60)).padStart(2, '0')}:${String(min % 60).padStart(2, '0')}${z}`;
  })();
  const labels = days.flatMap((d, r) => (r === 0 || new Date(d * 86_400_000).getUTCMonth() !== new Date(days[r - 1]! * 86_400_000).getUTCMonth() ? [{ r, label: isoDay(d).slice(0, 7) }] : []));
  const every = Math.max(1, Math.ceil(labels.length / Math.max(1, Math.floor((height * rowPx) / 22))));
  const hours = Array.from({ length: 9 }, (_, i) => i * 6);

  return (
    <div data-testid="chart" style={{ position: 'relative' }}>
      <div style={{ marginLeft: GUTTER, height: 18, position: 'relative', fontFamily: 'var(--mono)', fontSize: 11, color: AX.stroke }}>
        {hours.map((h) => <span key={h} style={{ position: 'absolute', left: `${(h / 48) * 100}%`, transform: 'translateX(-50%)' }}>{String(h % 24).padStart(2, '0')}</span>)}
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: `${GUTTER}px 1fr` }}>
        <svg width={GUTTER} height={height * rowPx} aria-hidden="true">
          {labels.filter((_, i) => i % every === 0).map((l) => <text key={l.r} x={GUTTER - 6} y={l.r * rowPx + 9} textAnchor="end" fontSize={10} fill={AX.stroke} fontFamily="JetBrains Mono, monospace">{l.label}</text>)}
        </svg>
        <div style={{ position: 'relative' }}>
          <canvas
            ref={canvas}
            role="img"
            aria-label={`Double-plotted actogram, period ${periodH} hours`}
            style={{ width: '100%', height: height * rowPx, display: 'block', background: PAL.panel2, cursor: 'crosshair' }}
            onMouseMove={(e) => setHover(at(e))}
            onMouseLeave={() => setHover(null)}
          />
          {[6, 12, 18, 24, 30, 36, 42].map((h) => <div key={h} style={{ position: 'absolute', top: 0, bottom: 0, left: `${(h / 48) * 100}%`, borderLeft: `1px ${h === 24 ? 'solid' : 'dotted'} ${PAL.muted}55`, pointerEvents: 'none' }} />)}
        </div>
      </div>
      <div className="mono muted" style={{ fontSize: 12, minHeight: 18, marginLeft: GUTTER }}>{tip || 'Hover for the date and time.'}</div>
    </div>
  );
}
