import { act, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useStationNameOverflow } from './useStationNameOverflow';

function Name({ name, editing = false }: { name: string; editing?: boolean }) {
  const label = useStationNameOverflow(name, editing);
  return <div>{!editing && <span ref={label}>{name}</span>}</div>;
}

afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });

describe('station name overflow qualification', () => {
  it('keeps both fitting real short names unqualified and only enables shrinking for an already clipped name', () => {
    let natural = 85;
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockReturnValue(176);
    vi.spyOn(HTMLElement.prototype, 'scrollWidth', 'get').mockImplementation(() => natural);
    const view = render(<Name name="本机 Chrome" />);
    expect(screen.getByText('本机 Chrome')).not.toHaveAttribute('data-name-overflow');
    natural = 99;
    view.rerender(<Name name="本机 Chrome-1" />);
    expect(screen.getByText('本机 Chrome-1')).not.toHaveAttribute('data-name-overflow');
    natural = 400;
    view.rerender(<Name name="超过原名称格宽度的合成工位" />);
    expect(screen.getByText('超过原名称格宽度的合成工位')).toHaveAttribute('data-name-overflow');
  });

  it('remeasures the original layout after a container resize and after leaving rename mode', () => {
    let natural = 400, available = 176, resize = () => {};
    const disconnect = vi.fn();
    vi.stubGlobal('ResizeObserver', class {
      constructor(callback: () => void) { resize = callback; }
      observe() {}
      disconnect = disconnect;
    });
    vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockImplementation(() => available);
    vi.spyOn(HTMLElement.prototype, 'scrollWidth', 'get').mockImplementation(() => natural);
    const view = render(<Name name="合成工位" />);
    expect(screen.getByText('合成工位')).toHaveAttribute('data-name-overflow');
    available = 400;
    act(() => resize());
    expect(screen.getByText('合成工位')).not.toHaveAttribute('data-name-overflow');
    view.rerender(<Name name="合成工位" editing />);
    expect(disconnect).toHaveBeenCalledOnce();
    natural = 99;
    view.rerender(<Name name="本机 Chrome-1" />);
    expect(screen.getByText('本机 Chrome-1')).not.toHaveAttribute('data-name-overflow');
  });
});
