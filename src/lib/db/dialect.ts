// Shared client-side SQL dialect helpers. Most SQL is generated on the Rust
// side (parameterized); this module is for the SQL editor and DDL builders.

export type Engine = 'mysql' | 'postgres' | 'sqlite';

export function defaultPort(engine: Engine): string {
  if (engine === 'postgres') return '5432';
  if (engine === 'mysql') return '3306';
  return '';
}

export function engineLabel(engine: Engine): string {
  if (engine === 'postgres') return 'PostgreSQL';
  if (engine === 'sqlite') return 'SQLite';
  return 'MySQL';
}

/** Quote an identifier (table/column) for the given engine. */
export function quoteIdent(engine: Engine, name: string): string {
  if (engine === 'mysql') return '`' + name.replace(/`/g, '``') + '`';
  return '"' + name.replace(/"/g, '""') + '"';
}

export const FILTER_OPS = ['=', '!=', '<', '>', '<=', '>=', 'LIKE'] as const;
export type FilterOp = (typeof FILTER_OPS)[number];
