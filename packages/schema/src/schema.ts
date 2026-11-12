import SCHEMA from '../../../schema/wellness-v1.json';

export type ColumnSpec = { name: string; type: string; [k: string]: unknown };
export const schema = SCHEMA as unknown as {
  schema_version: number;
  tables: Record<string, { columns: ColumnSpec[] }> | string[];
  series: Record<string, { columns: ColumnSpec[] }> | string[];
  rr: unknown;
};
export default schema;
