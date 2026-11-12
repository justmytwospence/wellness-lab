import { describe, expect, it } from 'vitest';
import { parquetReadObjects } from 'hyparquet';
import { memoryFiles } from './fs';
import { splitMonths, writeExport } from './write';
import type { ExportResultLike } from './types';

describe('splitMonths', () => {
  it('splits at UTC month boundaries', () => {
    const t = Float64Array.from([Date.UTC(2026, 0, 31, 23), Date.UTC(2026, 1, 1, 0), Date.UTC(2026, 1, 2), Date.UTC(2026, 2, 1)]);
    expect(splitMonths(t)).toEqual([{ month: '2026-01', from: 0, to: 1 }, { month: '2026-02', from: 1, to: 3 }, { month: '2026-03', from: 3, to: 4 }]);
  });
});

describe('writeExport', () => {
  it('writes months, a manifest with sha256 and readable Parquet', async () => {
    const t = Float64Array.from([Date.UTC(2026, 0, 31, 23), Date.UTC(2026, 1, 1), Date.UTC(2026, 1, 2)]);
    const res: ExportResultLike = {
      tables: () => [], columns: () => [], numeric: () => new Float64Array(), strings: () => '[]',
      series: () => ['hr'], seriesColumn: (_n, c) => (c === 't' ? t : Float64Array.from([60, 61, NaN])),
      rrIds: () => ['garmin-1'], rr: () => Float32Array.from([800, 810]), report: () => '{}',
    };
    const fs = memoryFiles();
    const m = await writeExport(res, fs, 'test');
    expect(m.files.map((f) => f.path)).toEqual(['rr/garmin-1.parquet', 'series/hr/2026-01.parquet', 'series/hr/2026-02.parquet']);
    expect(m.files.every((f) => /^[0-9a-f]{64}$/.test(f.sha256))).toBe(true);
    const bytes = (await fs.read(['lab', 'wellness-v1', 'series', 'hr', '2026-02.parquet']))!;
    const rows = await parquetReadObjects({ file: bytes });
    expect(rows.map((r: Record<string, unknown>) => [Number(r.t), r.v])).toEqual([[t[1], 61], [t[2], null]]);
  });
});
