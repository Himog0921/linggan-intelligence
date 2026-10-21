import { useCallback, useState } from 'react';

/** 采集页读同一个键；键名、数组格式与跳转形状在阶段 G 完成前冻结（DR-6）。 */
export function selectionKey(domain: string): string {
  return `collection-selected:${domain}`;
}

function read(domain: string | null): Set<string> {
  if (domain === null) return new Set();
  try {
    const value: unknown = JSON.parse(sessionStorage.getItem(selectionKey(domain)) ?? '[]');
    return new Set(Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : []);
  } catch {
    return new Set();
  }
}

function write(domain: string, selection: ReadonlySet<string>) {
  try {
    sessionStorage.setItem(selectionKey(domain), JSON.stringify([...selection]));
  } catch {
    /* 存储不可用时勾选仍在内存里可用，只是刷新后不保留。 */
  }
}

/**
 * 批量操作的勾选集合：独立于检查器的选中；每次变化写入 sessionStorage，刷新、继续读取、换检索条件都保留。
 * 只存作品稳定引用（标识，不是内容）。领域一变就是另一份集合。
 */
export function useCollectionSelection(domain: string | null) {
  const [state, setState] = useState(() => ({ domain, selection: read(domain) }));
  // 领域变了就是另一份集合：渲染期间重置，而不是在 Effect 里补。
  const current = state.domain === domain ? state.selection : read(domain);
  if (state.domain !== domain) setState({ domain, selection: current });

  const update = useCallback(
    (change: (selection: Set<string>) => void) => {
      if (domain === null) return;
      setState(previous => {
        const next = new Set(previous.domain === domain ? previous.selection : read(domain));
        change(next);
        write(domain, next);
        return { domain, selection: next };
      });
    },
    [domain],
  );

  const toggle = useCallback((ref: string, checked: boolean) => update(selection => void (checked ? selection.add(ref) : selection.delete(ref))), [update]);
  const addAll = useCallback((refs: readonly string[]) => update(selection => refs.forEach(ref => selection.add(ref))), [update]);
  /** One toggle affects only loaded refs. Other filters' selections remain untouched. */
  const togglePage = useCallback((refs: readonly string[]) => update(selection => {
    if (refs.length === 0) return;
    const fullySelected = refs.every(ref => selection.has(ref));
    for (const ref of refs) {
      if (fullySelected) selection.delete(ref);
      else selection.add(ref);
    }
  }), [update]);
  const clear = useCallback(() => update(selection => selection.clear()), [update]);

  return { selection: current, toggle, addAll, togglePage, clear };
}
