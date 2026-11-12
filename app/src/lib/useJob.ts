import { useEffect, useRef, useState } from 'react';
import type { Job, ResultOf } from '@wl/schema';
import type { Run } from '@wl/engine';
import { useApp } from './app';

export interface JobState<J extends Job> { data: Run<J> | null; loading: boolean; error: string | null }

/** Run an engine job whenever it (or the data version) changes; null skips. Superseded jobs are dropped. */
export function useJob<J extends Job>(job: J | null): JobState<J> {
  const { engine, version } = useApp();
  const [st, setSt] = useState<JobState<J>>({ data: null, loading: !!job, error: null });
  const key = job ? JSON.stringify(job) : '';
  const seq = useRef(0);
  useEffect(() => {
    if (!job) return setSt({ data: null, loading: false, error: null });
    const mine = ++seq.current;
    setSt((s) => ({ ...s, loading: true, error: null }));
    engine.run(job, { alive: () => mine === seq.current }).then(
      (data) => mine === seq.current && setSt({ data, loading: false, error: null }),
      (e: Error) => mine === seq.current && e.message !== 'superseded' && setSt({ data: null, loading: false, error: e.message }),
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, version, engine]);
  return st;
}
export type { ResultOf };
