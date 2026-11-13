import { useEffect, useRef, useState } from 'react';

export interface RasterProps {
  rgba: Uint8Array;
  width: number;
  height: number;
  /** pixel height of one row */
  rowPx?: number;
  onHover?(row: number, col: number): void;
  onClick?(row: number, col: number): void;
  onSelect?(r0: number, r1: number): void;
  overlay?: React.ReactNode;
  /** fill the container's height instead of rowPx per row */
  fit?: boolean;
}

/** An engine-drawn RGBA raster (rows are nights, columns time bins), stretched to the container width. */
export function Raster({ rgba, width, height, rowPx = 3, onHover, onClick, onSelect, fit }: RasterProps) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [drag, setDrag] = useState<[number, number] | null>(null);
  useEffect(() => {
    const c = canvas.current!;
    c.width = width;
    c.height = height;
    c.getContext('2d')!.putImageData(new ImageData(new Uint8ClampedArray(rgba.buffer as ArrayBuffer, rgba.byteOffset, rgba.byteLength), width, height), 0, 0);
  }, [rgba, width, height]);
  const at = (e: React.MouseEvent) => {
    const r = canvas.current!.getBoundingClientRect();
    return [Math.min(height - 1, Math.max(0, Math.floor(((e.clientY - r.top) / r.height) * height))), Math.min(width - 1, Math.max(0, Math.floor(((e.clientX - r.left) / r.width) * width)))] as const;
  };
  return (
    <div style={{ position: 'relative', width: '100%', height: fit ? '100%' : undefined }}>
      <canvas
        ref={canvas}
        style={{ width: '100%', height: fit ? '100%' : height * rowPx, imageRendering: 'pixelated', display: 'block', cursor: 'crosshair' }}
        onMouseMove={(e) => {
          const [r, c] = at(e);
          onHover?.(r, c);
          if (drag) setDrag([drag[0], r]);
        }}
        onMouseDown={(e) => e.shiftKey && setDrag([at(e)[0], at(e)[0]])}
        onMouseUp={(e) => {
          if (drag) {
            const [a, b] = [Math.min(drag[0], at(e)[0]), Math.max(drag[0], at(e)[0])];
            setDrag(null);
            onSelect?.(a, b);
          }
        }}
        onClick={(e) => {
          if (e.shiftKey) return;
          const [r, c] = at(e);
          onClick?.(r, c);
        }}
      />
      {drag && <div style={{ position: 'absolute', left: 0, right: 0, top: `${(Math.min(...drag) / height) * 100}%`, height: `${((Math.abs(drag[1] - drag[0]) + 1) / height) * 100}%`, background: '#7aa2ff40', border: '1px solid #7aa2ff', pointerEvents: 'none' }} />}
    </div>
  );
}
