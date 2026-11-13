import { useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';

export interface Size { w: number; h: number }

/** The measured content box of an element, in whole pixels (0 until laid out). */
export function useSize<T extends HTMLElement>(): [React.RefObject<T | null>, Size] {
  const ref = useRef<T>(null);
  const [s, setS] = useState<Size>({ w: 0, h: 0 });
  useLayoutEffect(() => {
    const el = ref.current!;
    const set = (w: number, h: number) => setS((p) => (p.w === w && p.h === h ? p : { w, h }));
    set(Math.floor(el.clientWidth), Math.floor(el.clientHeight));
    const ro = new ResizeObserver((e) => {
      const r = e[0]!.contentRect;
      set(Math.floor(r.width), Math.floor(r.height));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return [ref, s];
}

/**
 * Takes the space its parent gives it and hands the pixel size to its child. The child sits in an absolutely
 * positioned box, so what it draws can never change the measurement.
 */
export function Fill({ children, className, style }: { children: (s: Size) => ReactNode; className?: string; style?: CSSProperties }) {
  const [ref, s] = useSize<HTMLDivElement>();
  return (
    <div className={`fill ${className ?? ''}`} style={style}>
      <div className="fill-in" ref={ref}>{s.w > 0 && s.h > 0 && children(s)}</div>
    </div>
  );
}

/** Charts that share one panel's height, top to bottom (each child is a `Fill` or plain content). */
export function Stack({ children, weights }: { children: ReactNode[]; weights?: number[] }) {
  return (
    <div className="stack-col">
      {children.map((c, i) => <div key={i} className="fill" style={{ flex: `${weights?.[i] ?? 1} 1 0` }}>{c}</div>)}
    </div>
  );
}
