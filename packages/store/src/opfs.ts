// Adapted from effort-curves packages/store/src/opfs.ts @ bef4120 (same author), relicensed under AGPL-3.0-only
/** The browser's private file system (OPFS): everything this app caches lives here, on this device only. */

async function dir(path: string[], create: boolean): Promise<FileSystemDirectoryHandle | null> {
  let d = await navigator.storage.getDirectory();
  for (const p of path) {
    try {
      d = await d.getDirectoryHandle(p, { create });
    } catch {
      return null;
    }
  }
  return d;
}

export async function readFile(path: string[]): Promise<ArrayBuffer | null> {
  const d = await dir(path.slice(0, -1), false);
  if (!d) return null;
  try {
    const f = await (await d.getFileHandle(path.at(-1)!)).getFile();
    return await f.arrayBuffer();
  } catch {
    return null;
  }
}

export async function writeFile(path: string[], data: ArrayBuffer | ArrayBufferView | string): Promise<void> {
  const d = (await dir(path.slice(0, -1), true))!;
  const h = await d.getFileHandle(path.at(-1)!, { create: true });
  const w = await h.createWritable();
  await w.write(data as FileSystemWriteChunkType);
  await w.close();
}

export async function removeEntry(path: string[]): Promise<void> {
  const d = await dir(path.slice(0, -1), false);
  if (!d) return;
  try {
    await d.removeEntry(path.at(-1)!, { recursive: true });
  } catch {
    // already gone
  }
}

export async function estimate(): Promise<StorageEstimate> {
  return navigator.storage.estimate();
}
