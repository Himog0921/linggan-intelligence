import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { useTitleTooltip, type TitleTruncation } from './TitleTooltip';

const resizeCallbacks: Array<() => void> = [];

class FakeResizeObserver {
  constructor(callback: () => void) {
    resizeCallbacks.push(callback);
  }
  observe() {}
  unobserve() {}
  disconnect() {}
}

beforeEach(() => {
  resizeCallbacks.length = 0;
  vi.stubGlobal('ResizeObserver', FakeResizeObserver);
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

function Harness({ text, truncation = 'width' }: { text: string; truncation?: TitleTruncation }) {
  const { anchorRef, onEnter, onLeave, onFocus, onBlur, tooltip } = useTitleTooltip<HTMLButtonElement>(text, truncation);
  return <>
    <button ref={anchorRef} onMouseEnter={onEnter} onMouseLeave={onLeave} onFocus={onFocus} onBlur={onBlur}>{text}</button>
    {tooltip}
  </>;
}

function setMetrics(element: HTMLElement, kind: TitleTruncation, visible: number, content: number) {
  Object.defineProperty(element, kind === 'width' ? 'clientWidth' : 'clientHeight', { configurable: true, value: visible });
  Object.defineProperty(element, kind === 'width' ? 'scrollWidth' : 'scrollHeight', { configurable: true, value: content });
}

const hint = () => document.querySelector<HTMLElement>('[data-title-tooltip]');

function advance(ms: number) {
  act(() => {
    vi.advanceTimersByTime(ms);
  });
}

it('shows the full title after a short hover delay only when the visible title is clipped', () => {
  render(<Harness text="一段被截断的长标题" />);
  const trigger = screen.getByRole('button');
  setMetrics(trigger, 'width', 120, 240);

  fireEvent.mouseEnter(trigger);
  expect(hint()).toBeNull();
  advance(300);
  expect(hint()).toHaveTextContent('一段被截断的长标题');
  expect(hint()).toHaveAttribute('aria-hidden', 'true');
});

it('does not show for a short title, and line-clamped cover titles use the height check', () => {
  render(<Harness text="短标题" />);
  const trigger = screen.getByRole('button');
  setMetrics(trigger, 'width', 240, 240);
  fireEvent.mouseEnter(trigger);
  advance(300);
  expect(hint()).toBeNull();

  const cover = render(<Harness text="两行封面的长标题" truncation="lines" />);
  const coverTrigger = cover.getByRole('button', { name: '两行封面的长标题' });
  setMetrics(coverTrigger, 'lines', 40, 60);
  fireEvent.mouseEnter(coverTrigger);
  advance(300);
  expect(hint()).toHaveTextContent('两行封面的长标题');
});

it('opens immediately on keyboard focus, closes on Escape, and can be opened again after refocusing', () => {
  render(<Harness text="键盘可以读到的长标题" />);
  const trigger = screen.getByRole('button');
  setMetrics(trigger, 'width', 100, 220);

  fireEvent.focus(trigger);
  expect(hint()).toHaveTextContent('键盘可以读到的长标题');

  fireEvent.keyDown(document, { key: 'Escape' });
  expect(hint()).toBeNull();

  fireEvent.blur(trigger);
  fireEvent.focus(trigger);
  expect(hint()).toHaveTextContent('键盘可以读到的长标题');
});

it('stays readable while the pointer moves onto the hint, then closes shortly after both are left', () => {
  render(<Harness text="可以移到提示上的长标题" />);
  const trigger = screen.getByRole('button');
  setMetrics(trigger, 'width', 100, 220);
  fireEvent.mouseEnter(trigger);
  advance(300);
  const tooltip = hint()!;

  fireEvent.mouseLeave(trigger);
  fireEvent.mouseEnter(tooltip);
  advance(200);
  expect(hint()).toBe(tooltip);

  fireEvent.mouseLeave(tooltip);
  advance(200);
  expect(hint()).toBeNull();
});

it('closes when a width change makes the full title fit, and follows resize while still clipped', () => {
  render(<Harness text="宽度变化后更新的长标题" />);
  const trigger = screen.getByRole('button');
  setMetrics(trigger, 'width', 100, 220);
  fireEvent.mouseEnter(trigger);
  advance(300);
  expect(hint()).not.toBeNull();

  setMetrics(trigger, 'width', 220, 220);
  act(() => {
    for (const callback of [...resizeCallbacks]) callback();
  });
  expect(hint()).toBeNull();

  setMetrics(trigger, 'width', 100, 220);
  fireEvent.mouseEnter(trigger);
  advance(300);
  expect(hint()).not.toBeNull();
  act(() => {
    for (const callback of [...resizeCallbacks]) callback();
  });
  expect(hint()).not.toBeNull();
});
