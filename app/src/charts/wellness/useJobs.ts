import { useEffect, useRef, useState } from 'react';
import type { Job } from '@wl/schema';
import { useApp } from '../../lib/app';

/** Run several engine jobs together (each is memoised by the engine); results line up with `jobs`, null while pending or failed. */
export function useJobs(jobs: Job[] | null): { data: (any | null)[]; loading: boolean } {
  const { engine, version } = useApp();
  const [st, setSt] = useState<{ data: (any | null)[]; loading: boolean }>({ data: [], loading: !!jobs });
  const key = jobs ? JSON.stringify(jobs) : '';
  const seq = useRef(0);
  useEffect(() => {
    if (!jobs) return setSt({ data: [], loading: false });
    const mine = ++seq.current;
    setSt((s) => ({ ...s, loading: true }));
    void Promise.allSettled(jobs.map((j) => engine.run(j, { alive: () => mine === seq.current }))).then((rs) => {
      if (mine === seq.current) setSt({ data: rs.map((r) => (r.status === 'fulfilled' ? r.value : null)), loading: false });
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, version, engine]);
  return st;
}
