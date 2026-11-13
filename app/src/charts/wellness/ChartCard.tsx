import type { ReactNode } from 'react';
import { Panel } from '../../ui/Panel';
import { Empty } from '../small';

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
  return <Panel title={title} sub={sub} method={method} job={job} needs={needs} loading={loading} error={error} badge={badge} actions={actions}>{children}</Panel>;
}

export function NeedsInput({ input }: { input: string }) {
  return <Empty><span data-testid="needs-input">Not available from this data source: needs {input}</span></Empty>;
}

export const Experimental = () => <span className="chip" style={{ background: 'var(--hr)', marginLeft: 8, fontSize: 11 }} title="Experimental: a model of the body clock, not a measurement">Experimental</span>;
