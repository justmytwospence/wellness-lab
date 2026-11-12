/// <reference lib="webworker" />
/** The import worker: wasm `ExportBuilder` over the dropped zip, progress posted as ImportEvents. */
import init, { ExportBuilder, version } from '../../engine/pkg/wellness_lab.js';
import { opfsFiles } from './fs';
import { importGarminExport } from './garminExport';
import type { ImportEvent } from './types';

self.onmessage = async (e: MessageEvent<{ file: Blob }>) => {
  const emit = (ev: ImportEvent) => postMessage(ev);
  try {
    await init();
    await importGarminExport(e.data.file, { files: opfsFiles, builder: new ExportBuilder() as never, emit, importer: `wellness-lab-import@${version()}` });
  } catch (err) {
    emit({ type: 'error', error: String((err as Error)?.message ?? err) });
  }
};
