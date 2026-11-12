import { BlobWriter, TextReader, Uint8ArrayReader, ZipWriter, ZipReader, BlobReader, Uint8ArrayWriter, TextWriter } from '@zip.js/zip.js/lib/zip-native.js';
import { readFile, writeFile } from './opfs';

export interface BackupFs {
  list(): Promise<string[][]>;
  read(p: string[]): Promise<ArrayBuffer | null>;
}

/** A zip of `lab/wellness-v1/**` and `lab/state.json`, for moving to another browser. */
export async function exportBackup(fs: BackupFs): Promise<Blob> {
  const zw = new ZipWriter(new BlobWriter('application/zip'));
  for (const p of await fs.list()) {
    const b = await fs.read(p);
    if (b) await zw.add(p.join('/'), new Uint8ArrayReader(new Uint8Array(b)));
  }
  await zw.add('backup.json', new TextReader(JSON.stringify({ app: 'wellness-lab', at: new Date().toISOString() })));
  return zw.close();
}

export async function importBackup(blob: Blob, write: (p: string[], d: Uint8Array | string) => Promise<void> = (p, d) => writeFile(p, d)): Promise<number> {
  const zr = new ZipReader(new BlobReader(blob));
  let n = 0;
  for (const e of await zr.getEntries()) {
    if (e.directory || e.filename === 'backup.json' || !e.filename.startsWith('lab/')) continue;
    const bytes = await e.getData!(new Uint8ArrayWriter());
    await write(e.filename.split('/'), bytes);
    n++;
  }
  await zr.close();
  void TextWriter;
  void readFile;
  return n;
}
