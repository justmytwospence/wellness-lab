/** A Garmin account export zip, streamed into the wasm builder, then written as wellness-v1 files. */
import { isFile, openZip, entryBytes, type FileEntry } from './zip';
import { TMP_DIR, type FileStore } from './fs';
import { writeExport } from './write';
import type { ExportBuilderLike, ImportEvent, ImportReport } from './types';
import { TextWriter } from '@zip.js/zip.js/lib/zip-native.js';

const squash = (p: string) => p.toLowerCase().replace(/\\/g, '/').replace(/[_-]/g, '');
const JSON_PATTERNS = [/udsfile[^/]*\.json$/, /sleepdata\.json$/, /healthstatusdata\.json$/, /trainingreadinessdto[^/]*\.json$/, /metricsmaxmetdata[^/]*\.json$/, /metricsheataltitudeacclimation[^/]*\.json$/, /lifestylelogging\.json$/, /summarizedactivities\.json$/];
export const isWanted = (name: string) => JSON_PATTERNS.some((r) => r.test(squash(name)));

export interface Ctx { files: FileStore; builder: ExportBuilderLike; emit(e: ImportEvent): void; signal?: AbortSignal; importer?: string }

export async function importGarminExport(src: Blob, ctx: Ctx): Promise<ImportReport> {
  const zip = openZip(src);
  const entries = (await zip.getEntries()).filter(isFile);
  const check = () => {
    if (ctx.signal?.aborted) throw new Error('cancelled');
  };
  // JSON first: activity files are matched to their summaries by start time, and local offsets come from the daily files
  ctx.emit({ type: 'phase', phase: 'json', message: 'Reading daily summaries' });
  const js = entries.filter((e) => isWanted(e.filename));
  for (let i = 0; i < js.length; i++) {
    check();
    const text = await js[i]!.getData!(new TextWriter());
    ctx.builder.addJson(js[i]!.filename, text);
    ctx.emit({ type: 'progress', done: i + 1, total: js.length, file: js[i]!.filename });
  }
  ctx.emit({ type: 'phase', phase: 'fit', message: 'Reading device files' });
  const inner = entries.filter((e) => /uploadedfiles[^/]*\.zip$/.test(squash(e.filename)));
  const totalBytes = inner.reduce((n, e) => n + e.compressedSize, 0) || 1;
  let doneBytes = 0;
  const loose = entries.filter((e) => e.filename.toLowerCase().endsWith('.fit'));
  for (const e of loose) {
    check();
    ctx.builder.addFit(e.filename, await entryBytes(e));
  }
  for (const e of inner) {
    check();
    // nested zips are read lazily from OPFS so memory holds one device file at a time
    await ctx.files.write([...TMP_DIR, 'part.zip'], await entryBytes(e));
    const buf = await ctx.files.read([...TMP_DIR, 'part.zip']);
    const nested = openZip(new Blob([buf!]));
    const fits = (await nested.getEntries()).filter((x): x is FileEntry => isFile(x) && x.filename.toLowerCase().endsWith('.fit'));
    for (let i = 0; i < fits.length; i++) {
      check();
      try {
        ctx.builder.addFit(fits[i]!.filename, await entryBytes(fits[i]!));
      } catch (err) {
        if (String((err as Error)?.message ?? err).includes('json-first')) throw err;
      }
      if (i % 50 === 0) ctx.emit({ type: 'progress', done: Math.round(doneBytes + (e.compressedSize * i) / fits.length), total: totalBytes, file: e.filename });
    }
    await nested.close();
    doneBytes += e.compressedSize;
    ctx.emit({ type: 'progress', done: doneBytes, total: totalBytes, file: e.filename });
  }
  await zip.close();
  ctx.emit({ type: 'phase', phase: 'write', message: 'Writing your data' });
  const result = ctx.builder.finish();
  const report = JSON.parse(result.report()) as ImportReport;
  await writeExport(result, ctx.files, ctx.importer ?? 'wellness-lab-import');
  ctx.emit({ type: 'done', report });
  return report;
}
