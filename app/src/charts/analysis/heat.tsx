import { useState } from 'react';
import { PAL } from '@wl/views';

export interface HeatProps {
  rows: string[];
  cols: number;
  colTicks?: { i: number; text: string }[];
  value(r: number, c: number): number | null;
  color(v: number, r: number, c: number): string;
  /** 0..1 opacity of a cell (default 1) */
  alpha?(r: number, c: number): number;
  tip(r: number, c: number, v: number | null): string;
  cw?: number;
  ch?: number;
  rowLabelW?: number;
  /** write each cell's value in it (small grids) */
  text?(v: number): string;
  mark?: [number, number] | null;
  onCell?(r: number, c: number): void;
  ariaLabel: string;
  testid?: string;
}

/** A plain SVG heat grid: labelled rows, columns by index, a hover readout underneath. */
export function Heatgrid({ rows, cols, colTicks, value, color, alpha, tip, cw = 14, ch = 18, rowLabelW = 90, text, mark, onCell, ariaLabel, testid }: HeatProps) {
  const [hover, setHover] = useState('');
  const top = colTicks ? 16 : 0;
  const width = rowLabelW + cols * cw, height = top + rows.length * ch;
  const cells: React.ReactNode[] = [];
  for (let r = 0; r < rows.length; r++) {
    for (let c = 0; c < cols; c++) {
      const v = value(r, c);
      const x = rowLabelW + c * cw, y = top + r * ch;
      cells.push(
        v === null || !Number.isFinite(v)
          ? <rect key={`${r}-${c}`} x={x} y={y} width={cw} height={ch} fill={PAL.panel2} opacity={0.5} />
          : <rect key={`${r}-${c}`} x={x} y={y} width={cw} height={ch} fill={color(v, r, c)} opacity={alpha ? alpha(r, c) : 1} />,
      );
      if (text && v !== null && Number.isFinite(v)) cells.push(<text key={`t${r}-${c}`} x={x + cw / 2} y={y + ch / 2 + 4} textAnchor="middle" fontSize={10} fill={PAL.text} style={{ pointerEvents: 'none' }}>{text(v)}</text>);
    }
  }
  return (
    <div data-testid={testid ?? 'heatgrid'} data-cells={rows.length * cols}>
      <div style={{ overflowX: 'auto' }}>
        <svg
          width={width} height={height} role="img" aria-label={ariaLabel} shapeRendering="crispEdges"
          onMouseMove={(e) => {
            const b = e.currentTarget.getBoundingClientRect();
            const c = Math.floor((e.clientX - b.left - rowLabelW) / cw), r = Math.floor((e.clientY - b.top - top) / ch);
            setHover(r >= 0 && r < rows.length && c >= 0 && c < cols ? tip(r, c, value(r, c)) : '');
          }}
          onMouseLeave={() => setHover('')}
          onClick={(e) => {
            const b = e.currentTarget.getBoundingClientRect();
            const c = Math.floor((e.clientX - b.left - rowLabelW) / cw), r = Math.floor((e.clientY - b.top - top) / ch);
            if (onCell && r >= 0 && r < rows.length && c >= 0 && c < cols) onCell(r, c);
          }}
          style={{ cursor: onCell ? 'pointer' : 'default' }}
        >
          {rows.map((n, r) => <text key={r} x={rowLabelW - 6} y={top + r * ch + ch / 2 + 4} textAnchor="end" fontSize={11} fill={PAL.muted}>{n}</text>)}
          {colTicks?.map((t) => <text key={t.i} x={rowLabelW + t.i * cw} y={11} fontSize={10} fill={PAL.muted}>{t.text}</text>)}
          {cells}
          {mark && <rect x={rowLabelW + mark[1] * cw} y={top + mark[0] * ch} width={cw} height={ch} fill="none" stroke={PAL.text} strokeWidth={2} />}
        </svg>
      </div>
      <div className="mono muted" style={{ fontSize: 11, minHeight: 16 }}>{hover}</div>
    </div>
  );
}
