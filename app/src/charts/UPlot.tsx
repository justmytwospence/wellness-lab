import { useEffect, useRef } from 'react';
import uPlot from 'uplot';

export interface UPlotProps {
  options: Omit<uPlot.Options, 'width' | 'height'>;
  data: uPlot.AlignedData;
  height?: number;
  /** sync key: cursors with the same key move together */
  sync?: string;
  onReady?(u: uPlot): void;
  className?: string;
}

/** uPlot sized to its container, redrawn when the data or options change. */
export function UPlot({ options, data, height = 160, sync, onReady, className }: UPlotProps) {
  const el = useRef<HTMLDivElement>(null);
  const plot = useRef<uPlot | null>(null);
  const opts = useRef(options);
  opts.current = options;
  const heightRef = useRef(height);
  heightRef.current = height;

  useEffect(() => {
    const host = el.current!;
    const o: uPlot.Options = { ...opts.current, width: host.clientWidth || 600, height: heightRef.current, cursor: { ...(opts.current.cursor ?? {}), sync: sync ? { key: sync } : undefined } };
    const u = new uPlot(o, data, host);
    plot.current = u;
    onReady?.(u);
    const ro = new ResizeObserver(() => u.setSize({ width: host.clientWidth, height: heightRef.current }));
    ro.observe(host);
    return () => {
      ro.disconnect();
      u.destroy();
      plot.current = null;
    };
    // the plot is rebuilt when its options object changes identity (callers memoise it)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [options, sync]);

  useEffect(() => {
    if (el.current && el.current.clientWidth > 0) plot.current?.setSize({ width: el.current.clientWidth, height });
  }, [height]);

  useEffect(() => {
    plot.current?.setData(data);
  }, [data]);

  return <div ref={el} className={className} style={{ width: '100%' }} />;
}

const css = (name: string, fallback: string) => (typeof document === 'undefined' ? fallback : getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback);
/** Axis styling from the theme tokens, read when a plot is built (spread it into an axis spec). */
export const axisStyle = {
  get stroke() { return css('--muted', '#8b9099'); },
  get grid() { return { stroke: css('--grid', '#1d2127'), width: 1 }; },
  get ticks() { return { stroke: css('--grid', '#1d2127'), width: 1 }; },
  font: '11px "JetBrains Mono", monospace',
};
