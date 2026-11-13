import type { CSSProperties, ReactNode } from 'react';
import { useApp } from '../lib/app';
import { MethodButton } from '../layout/MethodPanel';
import { Grade } from '../charts/small';

export interface PanelProps {
  title?: ReactNode;
  sub?: ReactNode;
  /** registry method id: adds the grade and the info button */
  method?: string;
  job?: unknown;
  badge?: ReactNode;
  actions?: ReactNode;
  /** a missing input: the content is replaced by the explanation */
  needs?: string | null;
  loading?: boolean;
  error?: string | null;
  /** the body scrolls inside the panel */
  scroll?: boolean;
  testid?: string;
  className?: string;
  style?: CSSProperties;
  children?: ReactNode;
}

/** One region of a view: a head (title, grade, actions, method button) over a body that fills the rest. */
export function Panel({ title, sub, method, job, badge, actions, needs, loading, error, scroll, testid, className, style, children }: PanelProps) {
  const { registry } = useApp();
  const grade = method ? registry?.methods.find((m) => m.id === method)?.validity : undefined;
  return (
    <section className={`panel${scroll ? ' scroll' : ''}${className ? ' ' + className : ''}`} data-testid={testid ?? 'chart-card'} style={style}>
      {(title || actions || method) && (
        <div className="panel-head">
          {title && <h2>{title}{grade && <Grade g={grade} />}{badge}</h2>}
          {sub && <span className="sub" title={typeof sub === 'string' ? sub : undefined}>{sub}</span>}
          <div className="actions">{actions}{method && <MethodButton id={method} job={job} />}</div>
        </div>
      )}
      <div className="panel-body">
        {needs ? <div className="empty"><span data-testid="needs-input">Not available from this data source: needs {needs}</span></div> : error ? <div className="err">{error}</div> : loading ? <div className="empty">Working...</div> : children}
      </div>
    </section>
  );
}
