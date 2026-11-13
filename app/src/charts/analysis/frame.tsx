import { useSyncExternalStore, type ReactNode } from 'react';
import { Panel } from '../../ui/Panel';
import { InspectorNotes } from '../../ui/Inspector';

export function NotAvailable({ input, inline }: { input: string; inline?: boolean }) {
  if (inline) return <span className="muted" role="note" data-testid="not-available">Not available from this data source: needs {input}</span>;
  return <div className="empty" role="note" data-testid="not-available">Not available from this data source: needs {input}</div>;
}

/** A panel with a title, the method button, optional actions, and the content. */
export function Card({ title, method, job, actions, children, testid, scroll }: { title: ReactNode; method?: string; job?: unknown; actions?: ReactNode; children: ReactNode; testid?: string; scroll?: boolean }) {
  return <Panel title={title} method={method} job={job} actions={actions} testid={testid ?? 'chart-card'} scroll={scroll}>{children}</Panel>;
}

export function JobNote({ loading, error }: { loading: boolean; error: string | null }) {
  if (error) return <div className="err mono" role="alert" style={{ fontSize: 12 }}>{error}</div>;
  if (loading) return <div className="muted" style={{ fontSize: 12 }}>Working...</div>;
  return null;
}

const query = '(max-width: 767px)';
const subscribe = (cb: () => void) => {
  const m = matchMedia(query);
  m.addEventListener('change', cb);
  return () => m.removeEventListener('change', cb);
};
/** The analysis views are desktop views: on a phone they say so and give their address. */
export function Desktop({ children }: { children: ReactNode }) {
  const small = useSyncExternalStore(subscribe, () => matchMedia(query).matches);
  if (!small) return <>{children}</>;
  return <div className="empty" data-testid="needs-larger-screen">This view needs a larger screen.<br /><span className="mono">{location.pathname + location.search}</span></div>;
}

export function Intro({ children }: { children: ReactNode }) {
  return <InspectorNotes><h2>About this view</h2><p>{children}</p></InspectorNotes>;
}
