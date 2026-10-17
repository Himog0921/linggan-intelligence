import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { usePanelWidthPreference } from './usePanelWidthPreference';

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe('检查器宽度偏好', () => {
  it('默认标准；记住用户选的宽度，下次读回', () => {
    const first = renderHook(() => usePanelWidthPreference());
    expect(first.result.current[0]).toBe('normal');
    act(() => first.result.current[1]('wide'));
    expect(first.result.current[0]).toBe('wide');
    expect(localStorage.getItem('linggan.evidence.inspectorWidth')).toBe('wide');
    expect(renderHook(() => usePanelWidthPreference()).result.current[0]).toBe('wide');
  });

  it('存储里是未知值或存储不可用：回标准，不弄坏面板', () => {
    localStorage.setItem('linggan.evidence.inspectorWidth', 'huge');
    expect(renderHook(() => usePanelWidthPreference()).result.current[0]).toBe('normal');
    vi.stubGlobal('localStorage', {
      getItem: () => {
        throw new Error('blocked');
      },
      setItem: () => {
        throw new Error('blocked');
      },
    });
    const blocked = renderHook(() => usePanelWidthPreference());
    expect(blocked.result.current[0]).toBe('normal');
    act(() => blocked.result.current[1]('focus'));
    expect(blocked.result.current[0]).toBe('focus');
  });
});
