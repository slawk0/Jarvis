import { getIntlLocale } from '$lib/formatLocale';

export type SortDir = 'asc' | 'desc';

export interface SortState<C extends string = string> {
  column: C;
  direction: SortDir;
}

export function nextSort<C extends string>(
  current: SortState<C>,
  column: C,
): SortState<C> {
  if (current.column === column) {
    return { column, direction: current.direction === 'asc' ? 'desc' : 'asc' };
  }
  return { column, direction: 'asc' };
}

export function cmpStr(a: string, b: string, dir: SortDir): number {
  const r = (a ?? '').localeCompare(b ?? '', getIntlLocale(), { numeric: true, sensitivity: 'base' });
  return dir === 'asc' ? r : -r;
}

export function cmpNum(a: number, b: number, dir: SortDir): number {
  const r = (a ?? 0) - (b ?? 0);
  return dir === 'asc' ? r : -r;
}

export function cmpBool(a: boolean, b: boolean, dir: SortDir): number {
  const av = a ? 1 : 0;
  const bv = b ? 1 : 0;
  const r = av - bv;
  return dir === 'asc' ? r : -r;
}

export function applySort<T, C extends string>(
  items: T[],
  sort: SortState<C>,
  accessors: Record<C, (item: T) => string | number | boolean>,
  options?: {
    dirsFirst?: (item: T) => boolean;
  },
): T[] {
  const accessor = accessors[sort.column];
  if (!accessor) return [...items];

  return [...items].sort((a, b) => {
    if (options?.dirsFirst) {
      const da = options.dirsFirst(a);
      const db = options.dirsFirst(b);
      if (da !== db) return da ? -1 : 1;
    }

    const va = accessor(a);
    const vb = accessor(b);

    if (typeof va === 'number' && typeof vb === 'number') {
      return cmpNum(va, vb, sort.direction);
    }
    if (typeof va === 'boolean' && typeof vb === 'boolean') {
      return cmpBool(va, vb, sort.direction);
    }
    return cmpStr(String(va ?? ''), String(vb ?? ''), sort.direction);
  });
}
