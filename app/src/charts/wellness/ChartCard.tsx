import type { ReactNode } from 'react';
import { useApp } from '../../lib/app';
import { MethodButton } from '../../layout/MethodPanel';
import { Empty, Grade } from '../small';

/** The wrapper for every chart: title, validity grade, the method button, and the missing-input message. */
export function ChartCard({ title, sub, method, job, needs, loading, error, badge, actions, children }: {
  title: string;
  sub?: ReactNode;
  /** registry method id */
  method: string;
  job?: unknown;
  /** a missing input: the chart is replaced by the explanation */
  needs?: string | null;
  loading?: boolean;
  error?: string | null;
  badge?: ReactNode;
  actions?: ReactNode;
  children?: ReactNode;
}) {
  const { registry } = useApp();
  const grade = registry?.methods.find((m) => m.id === method)?.validity;
  return (
    <div className="card" data-testid="chart-card">
      <div className="row" style={{ justifyContent: 'space-between', alignItems: 'flex-start', flexWrap: 'nowrap' }}>
        <div style={{ minWidth: 0 }}>
          <h2 style={{ margin: 0 }}>{title}{grade && <Grade g={grade} />}{badge}</h2>
          {sub && <div className="muted" style={{ fontSize: 12, marginTop: 2 }}>{sub}</div>}
        </div>
        <div className="row" style={{ flexShrink: 0, justifyContent: 'flex-end' }}>{actions}<MethodButton id={method} job={job} /></div>
      </div>
      <div style={{ marginTop: 8 }}>
        {needs ? <NeedsInput input={needs} /> : error ? <div className="err">{error}</div> : loading ? <div className="empty">Working...</div> : children}
      </div>
    </div>
  );
}

export function NeedsInput({ input }: { input: string }) {
  return <Empty><span data-testid="needs-input">Not available from this data source: needs {input}</span></Empty>;
}

export const Experimental = () => <span className="chip" style={{ background: 'var(--hr)', marginLeft: 8, fontSize: 11 }} title="Experimental: a model of the body clock, not a measurement">Experimental</span>;
