import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { selectionKey, useCollectionSelection } from './useCollectionSelection';
import { DOMAIN, OTHER_DOMAIN, workRef } from './evidenceFixtures';

afterEach(() => sessionStorage.clear());

describe('勾选集合（与采集页的交接，C12／F06）', () => {
  it('键名是 collection-selected:<领域>，值是作品稳定引用的 JSON 数组；每次变化都写入', () => {
    const { result } = renderHook(() => useCollectionSelection(DOMAIN));
    expect(result.current.selection.size).toBe(0);
    act(() => result.current.toggle(workRef(1), true));
    act(() => result.current.addAll([workRef(2), workRef(3), workRef(1)]));
    expect(selectionKey(DOMAIN)).toBe(`collection-selected:${DOMAIN}`);
    expect(JSON.parse(sessionStorage.getItem(`collection-selected:${DOMAIN}`) ?? 'null')).toEqual([workRef(1), workRef(2), workRef(3)]);
    act(() => result.current.toggle(workRef(2), false));
    expect(JSON.parse(sessionStorage.getItem(`collection-selected:${DOMAIN}`) ?? 'null')).toEqual([workRef(1), workRef(3)]);
  });

  it('刷新后读回；存储里不是数组或含非字符串时忽略坏项，不崩', () => {
    sessionStorage.setItem(`collection-selected:${DOMAIN}`, JSON.stringify([workRef(1), 5, null, workRef(2)]));
    const { result } = renderHook(() => useCollectionSelection(DOMAIN));
    expect([...result.current.selection]).toEqual([workRef(1), workRef(2)]);
    sessionStorage.setItem(`collection-selected:${OTHER_DOMAIN}`, '{bad json');
    expect(renderHook(() => useCollectionSelection(OTHER_DOMAIN)).result.current.selection.size).toBe(0);
  });

  it('每个领域一份：换领域不带走上一个领域的勾选', () => {
    sessionStorage.setItem(`collection-selected:${DOMAIN}`, JSON.stringify([workRef(1)]));
    const { result, rerender } = renderHook(({ domain }) => useCollectionSelection(domain), { initialProps: { domain: DOMAIN as string | null } });
    expect(result.current.selection.has(workRef(1))).toBe(true);
    rerender({ domain: OTHER_DOMAIN });
    expect(result.current.selection.size).toBe(0);
    act(() => result.current.toggle(workRef(9), true));
    expect(sessionStorage.getItem(`collection-selected:${DOMAIN}`)).toBe(JSON.stringify([workRef(1)]));
    expect(JSON.parse(sessionStorage.getItem(`collection-selected:${OTHER_DOMAIN}`) ?? 'null')).toEqual([workRef(9)]);
  });

  it('没有领域：没有集合，也不写任何存储', () => {
    const { result } = renderHook(() => useCollectionSelection(null));
    act(() => result.current.toggle(workRef(1), true));
    expect(result.current.selection.size).toBe(0);
    expect(sessionStorage.length).toBe(0);
  });
});
