import { useSyncExternalStore, type ReactNode } from 'react';
import { MethodButton } from '../../layout/MethodPanel';

export function NotAvailable({ input, inline }: { input: string; inline?: boolean }) {
  if (inline) return <span className="muted" role="note" data-testid="not-available">Not available from this data source: needs {input}</span>;
  return <div className="empty" role="note" data-testid="not-available">Not available from this data source: needs {input}</div>;
}

/** A chart card: title, the method button, optional actions, and the content. */
export function Card({ title, method, job, actions, children, testid }: { title: ReactNode; method?: string; job?: unknown; actions?: ReactNode; children: ReactNode; testid?: string }) {
  return (
    <section className="card" data-testid={testid}>
      <div className="row" style={{ justifyContent: 'space-between', marginBottom: 6 }}>
        <h2 style={{ margin: 0 }}>{title} {method && <MethodButton id={method} job={job} />}</h2>
        {actions && <div className="row">{actions}</div>}
      </div>
      {children}
    </section>
  );
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
  return <p className="muted" style={{ maxWidth: '90ch' }}>{children}</p>;
}
