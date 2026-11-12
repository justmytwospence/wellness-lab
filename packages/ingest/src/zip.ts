// Adapted from effort-curves packages/ingest/src/zip.ts @ bef4120 (same author), relicensed under AGPL-3.0-only
/**
 * zip.js's native build (CompressionStream: no wasm and no blob: workers, which the app's CSP forbids), with web
 * workers off. Everything in this package imports zip.js from here.
 */
import { configure } from '@zip.js/zip.js/lib/zip-native.js';

configure({ useWebWorkers: false });

export {
  BlobReader,
  BlobWriter,
  TextReader,
  TextWriter,
  Uint8ArrayReader,
  Uint8ArrayWriter,
  ZipReader,
  ZipWriter,
} from '@zip.js/zip.js/lib/zip-native.js';
export type { Entry, FileEntry, Reader } from '@zip.js/zip.js';

import { BlobReader, Uint8ArrayWriter, ZipReader } from '@zip.js/zip.js/lib/zip-native.js';
import type { FileEntry, Reader } from '@zip.js/zip.js';

/** A ZIP to read: a Blob/File (read lazily) or any zip.js reader. */
export type ZipSource = Blob | Reader<unknown>;

export function openZip(src: ZipSource): ZipReader<unknown> {
  return new ZipReader(src instanceof Blob ? new BlobReader(src) : src);
}

export const isFile = (e: { directory: boolean }): e is FileEntry => !e.directory;

/** Entry bytes (small files only). */
export async function entryBytes(e: FileEntry): Promise<Uint8Array> {
  return e.getData(new Uint8ArrayWriter());
}

/** Entry data as a stream. */
export function entryStream(e: FileEntry): ReadableStream<Uint8Array> {
  const ts = new TransformStream<Uint8Array, Uint8Array>();
  e.getData(ts.writable).catch((err) => ts.writable.abort(err).catch(() => {}));
  return ts.readable;
}

export async function gunzip(bytes: Uint8Array): Promise<Uint8Array> {
  const stream = new Blob([bytes as BlobPart]).stream().pipeThrough(new DecompressionStream('gzip'));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

export const isGzip = (b: Uint8Array): boolean => b.length > 2 && b[0] === 0x1f && b[1] === 0x8b;
