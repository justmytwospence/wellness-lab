import type { ReactNode } from 'react';

/** The frame of every view: a toolbar (title, one-line description, tools), an optional stat strip, and a body that fills the rest. */
export function View({ title, desc, tools, strip, children, className, testid }: { title: ReactNode; desc?: string; tools?: ReactNode; strip?: ReactNode; children: ReactNode; className?: string; testid?: string }) {
  return (
    <div className={`view${strip ? ' has-strip' : ''}`} data-testid={testid}>
      <header className="toolbar">
        <h1>{title}</h1>
        {desc && <span className="desc" title={desc}>{desc}</span>}
        {tools && <div className="tools">{tools}</div>}
      </header>
      {strip}
      <div className={`view-body ${className ?? ''}`}>{children}</div>
    </div>
  );
}

export function ViewEmpty({ title, children }: { title: ReactNode; children: ReactNode }) {
  return <View title={title}><div className="empty" style={{ alignSelf: 'center' }}>{children}</div></View>;
}
