export interface ExportResultLike {
  tables(): string[];
  columns(table: string): string[];
  numeric(table: string, col: string): Float64Array;
  /** JSON array of (string|null) */
  strings(table: string, col: string): string;
  series(): string[];
  seriesColumn(name: string, col: string): Float64Array;
  rrIds(): string[];
  rr(id: string): Float32Array;
  report(): string;
}
export interface ExportBuilderLike {
  addJson(path: string, text: string): string;
  addFit(name: string, bytes: Uint8Array): string;
  finish(): ExportResultLike;
}
export type ImportEvent =
  | { type: 'phase'; phase: 'json' | 'fit' | 'write'; message: string }
  | { type: 'progress'; done: number; total: number; file: string }
  | { type: 'done'; report: ImportReport }
  | { type: 'error'; error: string };
export interface ImportReport {
  json_files: Record<string, number>;
  fit_files: Record<string, number>;
  errors: string[];
  skipped: number;
  days: [number, number] | null;
  series_rows: Record<string, number>;
  activities_with_rr: number;
}
