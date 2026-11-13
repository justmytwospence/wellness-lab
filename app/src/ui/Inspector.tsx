import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { X } from 'lucide-react';
import { useApp } from '../lib/app';
import { MethodDetails } from '../layout/MethodPanel';

interface Ctx { slot: HTMLElement | null; setSlot(el: HTMLElement | null): void; count: number; register(): () => void }
const InspectorCtx = createContext<Ctx>({ slot: null, setSlot() {}, count: 0, register: () => () => {} });

export function InspectorProvider({ children }: { children: ReactNode }) {
  const [slot, setSlot] = useState<HTMLElement | null>(null);
  const [count, setCount] = useState(0);
  const register = useCallback(() => {
    setCount((c) => c + 1);
    return () => setCount((c) => c - 1);
  }, []);
  const value = useMemo(() => ({ slot, setSlot, count, register }), [slot, count, register]);
  return <InspectorCtx.Provider value={value}>{children}</InspectorCtx.Provider>;
}

/** Notes for the current view: shown in the inspector drawer (toggle `i`), nowhere else. */
export function InspectorNotes({ children }: { children: ReactNode }) {
  const { slot, register } = useContext(InspectorCtx);
  useEffect(() => register(), [register]);
  return slot ? createPortal(<div className="notes" data-testid="notes">{children}</div>, slot) : null;
}

export function InspectorPanel() {
  const { ui } = useApp();
  const { count, setSlot } = useContext(InspectorCtx);
  const show = ui.inspector && (count > 0 || ui.method);
  // the slot must exist while notes mount, so it is always rendered once the drawer is open
  if (!ui.inspector) return null;
  return (
    <aside className="inspector" aria-label="Inspector" style={show ? undefined : { display: 'none' }}>
      <div className="ihead"><h3>Inspector</h3><button className="icon ghost" aria-label="Close inspector" onClick={() => ui.setInspector(false)}><X size={14} /></button></div>
      {ui.method && (
        <section aria-label="Method">
          <div className="ihead"><span /><button className="icon ghost" aria-label="Close method" onClick={() => ui.setMethod(null)}><X size={14} /></button></div>
          <MethodDetails />
          <hr style={{ border: 0, borderTop: '1px solid var(--line)', margin: '14px 0' }} />
        </section>
      )}
      <div ref={setSlot} />
    </aside>
  );
}
