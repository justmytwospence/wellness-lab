import { expect, it } from 'vitest';
import type { Proposal } from '@wl/schema';
import { LocalStateStore } from './state';

const files = new Map<string, string>();
const mk = () => new LocalStateStore(async (p) => { const s = files.get(p.join('/')); return s ? new TextEncoder().encode(s).buffer as ArrayBuffer : null; }, async (p, d) => void files.set(p.join('/'), d));
const prop = (): Proposal => ({ id: 'illness:100', kind: 'illness', start_day: 100, end_day: 102, posterior: { illness: 0.6 }, evidence: { nights: 3 } as never, score: 5, priority: 1, status: 'open', auto: false, asked: true, created_at: '', model_version: 'classify-1' });

it('confirm makes a label, undo removes it, deny keeps answers across redetection', async () => {
  files.clear();
  const s = mk();
  await s.saveProposals([prop()]);
  const r = await s.answer('illness:100', 'confirm');
  expect(r.label?.exclude_baseline).toBe(true);
  expect(r.label?.source).toBe('confirmed');
  let st = await s.load();
  expect(st.labels).toHaveLength(1);
  await s.undo('illness:100', r.label!.id);
  st = await s.load();
  expect(st.labels).toHaveLength(0);
  expect(st.proposals[0]!.status).toBe('open');
  await s.answer('illness:100', 'deny');
  await s.saveProposals([prop()]);
  st = await s.load();
  expect(st.proposals).toHaveLength(1);
  expect(st.proposals[0]!.status).toBe('denied');
  // persisted
  expect((await mk().load()).proposals[0]!.status).toBe('denied');
});
