import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { scaleLinear, scaleLog, scaleSqrt, type ScaleContinuousNumeric } from 'd3-scale';

export type Scale = ScaleContinuousNumeric<number, number>;
export interface AxisSpec {
  domain: [number, number];
  scale?: 'linear' | 'log' | 'sqrt';
  label?: string;
  /** a tick count for d3, or the exact ticks */
  ticks?: number | { v: number; label?: string }[];
  format?: (v: number) => string;
  /** positions outside the domain are pinned to the edge */
  clamp?: boolean;
  noGrid?: boolean;
}
export interface PlotCtx { sx: Scale; sy: Scale; w: number; h: number }
export interface PlotProps {
  height?: number;
  x: AxisSpec;
  y: AxisSpec;
  margin?: Partial<{ t: number; r: number; b: number; l: number }>;
  /** canvas layer under the SVG (for thousands of points) */
  draw?: (ctx: CanvasRenderingContext2D, c: PlotCtx) => void;
  /** SVG layer in the plot area (clipped) */
  children?: (c: PlotCtx) => ReactNode;
  /** SVG layer above the clip (labels that may overhang the plot area) */
  overlay?: (c: PlotCtx) => ReactNode;
  label?: string;
}

const make = (a: AxisSpec, range: [number, number]): Scale => {
  const s: Scale = a.scale === 'log' ? scaleLog() : a.scale === 'sqrt' ? scaleSqrt() : scaleLinear();
  s.domain(a.domain).range(range);
  if (a.clamp) s.clamp(true);
  return s;
};

/** A measured-width chart frame: a scaled canvas and an SVG layer with grid, axes and whatever the caller draws. */
export function Plot({ height = 240, x, y, margin, draw, children, overlay, label }: PlotProps) {
  const host = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const [W, setW] = useState(600);
  const clip = useId();
  useLayoutEffect(() => {
    const el = host.current!;
    setW(el.clientWidth || 600);
    const ro = new ResizeObserver(() => setW(el.clientWidth || 600));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const m = { t: 6, r: 12, b: x.label ? 36 : 22, l: y.label ? 54 : 44, ...margin };
  const w = Math.max(60, W - m.l - m.r), h = Math.max(40, height - m.t - m.b);
  const sx = make(x, [0, w]), sy = make(y, [h, 0]);
  const c: PlotCtx = { sx, sy, w, h };

  useEffect(() => {
    const cv = canvas.current;
    if (!cv || !draw) return;
    const dpr = window.devicePixelRatio || 1;
    cv.width = Math.round(w * dpr);
    cv.height = Math.round(h * dpr);
    const g = cv.getContext('2d')!;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, w, h);
    draw(g, c);
    // c is rebuilt on every render from the axis specs: redraw when they or the drawing change
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [draw, w, h, x.domain[0], x.domain[1], y.domain[0], y.domain[1], x.scale, y.scale]);

  const ticks = (a: AxisSpec, s: Scale, n: number): { v: number; label: string }[] => {
    if (Array.isArray(a.ticks)) return a.ticks.map((t) => ({ v: t.v, label: t.label ?? String(t.v) }));
    const count = typeof a.ticks === 'number' ? a.ticks : n;
    const fmt = a.format ?? (a.scale === 'log' ? (v: number) => String(v) : (s.tickFormat(count) as (v: number) => string));
    return s.ticks(count).map((v) => ({ v, label: fmt(v) }));
  };
  const xt = ticks(x, sx, Math.max(3, Math.floor(w / 90)));
  const yt = ticks(y, sy, Math.max(3, Math.floor(h / 40)));
  const txt = { fill: 'var(--muted)', fontSize: 11, fontFamily: 'var(--mono)' } as const;
  return (
    <div ref={host} style={{ position: 'relative', width: '100%', height }}>
      {draw && <canvas ref={canvas} style={{ position: 'absolute', left: m.l, top: m.t, width: w, height: h }} />}
      <svg data-chart width={W} height={height} style={{ position: 'absolute', left: 0, top: 0 }} role="img" aria-label={label}>
        <defs><clipPath id={clip}><rect x={0} y={0} width={w} height={h} /></clipPath></defs>
        <g transform={`translate(${m.l},${m.t})`}>
          {yt.map((t) => (
            <g key={`y${t.v}`} transform={`translate(0,${sy(t.v)})`}>
              {!y.noGrid && <line x2={w} stroke="var(--grid)" strokeWidth={1} opacity={0.7} />}
              <text x={-6} dy="0.32em" textAnchor="end" style={txt}>{t.label}</text>
            </g>
          ))}
          {xt.map((t) => (
            <g key={`x${t.v}`} transform={`translate(${sx(t.v)},${h})`}>
              {!x.noGrid && <line y2={-h} stroke="var(--grid)" strokeWidth={1} opacity={0.4} />}
              <text y={14} textAnchor="middle" style={txt}>{t.label}</text>
            </g>
          ))}
          {x.label && <text x={w / 2} y={h + 31} textAnchor="middle" style={txt}>{x.label}</text>}
          {y.label && <text transform={`translate(${-m.l + 12},${h / 2}) rotate(-90)`} textAnchor="middle" style={txt}>{y.label}</text>}
          <g clipPath={`url(#${clip})`}>{children?.(c)}</g>
          {overlay?.(c)}
        </g>
      </svg>
    </div>
  );
}
