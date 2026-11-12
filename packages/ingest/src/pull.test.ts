import { expect, it } from 'vitest';
import type { Manifest } from '@wl/schema';
import { memoryFiles, LIBRARY_DIR } from './fs';
import { pullWellness } from './pull';

const man = (files: Record<string, string>): Manifest => ({ schema_version: 1, generated_at: 'x', importer: 't', source: { kind: 'garmin-mcp' }, files: Object.entries(files).map(([path, sha256]) => ({ path, sha256, bytes: 1, rows: 1 })) });

it('fetches changed files, skips unchanged, removes deleted', async () => {
  const fs = memoryFiles();
  let remote = man({ 'days.parquet': 'a', 'series/hr/2026-01.parquet': 'b' });
  const served: string[] = [];
  const f = (async (url: string) => {
    const p = url.split('/v1/')[1]!;
    if (p === 'manifest.json') return new Response(JSON.stringify(remote));
    served.push(p);
    return new Response(new Uint8Array([1, 2, 3]));
  }) as unknown as typeof fetch;
  const o = { base: 'http://x/export/wellness/v1', files: fs, fetch: f };
  o.base = 'http://x/v1';
  expect((await pullWellness(o)).changed).toEqual(['days.parquet', 'series/hr/2026-01.parquet']);
  served.length = 0;
  remote = man({ 'days.parquet': 'a2', 'series/hr/2026-02.parquet': 'c' });
  const r = await pullWellness(o);
  expect(served.sort()).toEqual(['days.parquet', 'series/hr/2026-02.parquet']);
  expect(r.removed).toEqual(['series/hr/2026-01.parquet']);
  expect(await fs.read([...LIBRARY_DIR, 'series', 'hr', '2026-01.parquet'])).toBeNull();
  served.length = 0;
  expect((await pullWellness(o)).changed).toEqual([]);
  expect(served).toEqual([]);
});
