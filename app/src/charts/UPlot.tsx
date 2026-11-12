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

  useEffect(() => {
    const host = el.current!;
    const o: uPlot.Options = { ...opts.current, width: host.clientWidth || 600, height, cursor: { ...(opts.current.cursor ?? {}), sync: sync ? { key: sync } : undefined } };
    const u = new uPlot(o, data, host);
    plot.current = u;
    onReady?.(u);
    const ro = new ResizeObserver(() => u.setSize({ width: host.clientWidth, height }));
    ro.observe(host);
    return () => {
      ro.disconnect();
      u.destroy();
      plot.current = null;
    };
    // the plot is rebuilt when its options object changes identity (callers memoise it)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [options, height, sync]);

  useEffect(() => {
    plot.current?.setData(data);
  }, [data]);

  return <div ref={el} className={className} style={{ width: '100%' }} />;
}

export const axisStyle = { stroke: '#8892b8', grid: { stroke: '#222c52', width: 1 }, ticks: { stroke: '#222c52', width: 1 }, font: '11px "JetBrains Mono", monospace' };
