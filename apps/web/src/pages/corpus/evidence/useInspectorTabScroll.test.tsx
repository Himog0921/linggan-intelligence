import { useRef } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { useInspectorTabScroll } from './useInspectorTabScroll';

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
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function fireResize() {
  for (const callback of [...resizeCallbacks]) callback();
}

function Harness({ work, tab, ready = true }: { work: string | null; tab: string; ready?: boolean }) {
  const body = useRef<HTMLDivElement>(null);
  useInspectorTabScroll(body, work, tab, ready);
  return <div ref={body} data-testid="body" />;
}

function clampScroll(body: HTMLElement, getMax: () => number) {
  let top = 0;
  Object.defineProperty(body, 'scrollTop', {
    configurable: true,
    get: () => top,
    set: (value: number) => {
      top = Math.max(0, Math.min(value, getMax()));
      body.dispatchEvent(new Event('scroll'));
    },
  });
  return () => top;
}

it('keeps a non-zero position per work and tab and does not leak it to another work', () => {
  const view = render(<Harness work="work-a" tab="overview" />);
  const body = screen.getByTestId('body');
  body.scrollTop = 600;
  fireEvent.scroll(body);

  view.rerender(<Harness work="work-a" tab="dossier" />);
  body.scrollTop = 0; // the dossier host clamps the shared body; its own surface keeps the reading position
  fireEvent.scroll(body);
  view.rerender(<Harness work="work-a" tab="overview" />);
  expect(body.scrollTop).toBe(600);

  body.scrollTop = 300;
  fireEvent.scroll(body);
  view.rerender(<Harness work="work-a" tab="trace" />);
  expect(body.scrollTop).toBe(0);
  body.scrollTop = 420;
  fireEvent.scroll(body);
  view.rerender(<Harness work="work-a" tab="overview" />);
  expect(body.scrollTop).toBe(300);

  view.rerender(<Harness work="work-b" tab="overview" />);
  expect(body.scrollTop).toBe(0);
  body.scrollTop = 180;
  fireEvent.scroll(body);
  view.rerender(<Harness work="work-a" tab="overview" />);
  expect(body.scrollTop).toBe(300);
  view.rerender(<Harness work="work-b" tab="overview" />);
  expect(body.scrollTop).toBe(180);
  view.rerender(<Harness work="work-b" tab="trace" />);
  expect(body.scrollTop).toBe(0);
});

it('keeps the remembered position while a detail is loading and restores it on return', () => {
  const view = render(<Harness work="work-a" tab="overview" />);
  const body = screen.getByTestId('body');
  body.scrollTop = 600;
  fireEvent.scroll(body);

  view.rerender(<Harness work="work-b" tab="overview" ready={false} />);
  body.scrollTop = 0; // empty loading panel clamps the shared body
  fireEvent.scroll(body);
  view.rerender(<Harness work="work-b" tab="overview" />);
  expect(body.scrollTop).toBe(0);

  view.rerender(<Harness work="work-a" tab="overview" ready={false} />);
  body.scrollTop = 0;
  fireEvent.scroll(body);
  view.rerender(<Harness work="work-a" tab="overview" />);
  expect(body.scrollTop).toBe(600);
});

it('restores the work position when delayed content finally grows tall enough', () => {
  const view = render(<Harness work="work-a" tab="overview" />);
  const body = screen.getByTestId('body');
  let maxScroll = 1000;
  const top = clampScroll(body, () => maxScroll);

  body.scrollTop = 580;
  view.rerender(<Harness work="work-b" tab="overview" />);
  maxScroll = 0; // B's panel is still loading
  view.rerender(<Harness work="work-a" tab="overview" />);
  expect(top()).toBe(0); // A's panel is still loading, so the target is clamped

  maxScroll = 1000; // A's delayed detail arrives and the content grows
  fireResize();
  expect(top()).toBe(580);

  view.rerender(<Harness work="work-b" tab="overview" />);
  expect(top()).toBe(0);
  view.rerender(<Harness work="work-a" tab="overview" />);
  expect(top()).toBe(580);
});

it('does not pull the reader back when they interact while delayed content is loading', () => {
  const view = render(<Harness work="work-a" tab="overview" />);
  const body = screen.getByTestId('body');
  let maxScroll = 1000;
  const top = clampScroll(body, () => maxScroll);

  body.scrollTop = 580;
  view.rerender(<Harness work="work-b" tab="overview" />);
  maxScroll = 0;
  view.rerender(<Harness work="work-a" tab="overview" />);
  expect(top()).toBe(0);

  fireEvent.wheel(body);
  maxScroll = 1000;
  fireResize();
  expect(top()).toBe(0);
});

it('resets to the top when no work is selected', () => {
  const view = render(<Harness work="work-a" tab="overview" />);
  const body = screen.getByTestId('body');
  body.scrollTop = 200;
  fireEvent.scroll(body);
  view.rerender(<Harness work={null} tab="overview" />);
  expect(body.scrollTop).toBe(0);
});
