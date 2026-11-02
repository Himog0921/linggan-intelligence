import { useRef } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';
import { useInspectorTabScroll } from './useInspectorTabScroll';

function Harness({ work, tab, ready = true }: { work: string | null; tab: string; ready?: boolean }) {
  const body = useRef<HTMLDivElement>(null);
  useInspectorTabScroll(body, work, tab, ready);
  return <div ref={body} data-testid="body" />;
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

it('restores a work position only after its detail is ready, and ignores the loading clamp', () => {
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

it('resets to the top when no work is selected', () => {
  const view = render(<Harness work="work-a" tab="overview" />);
  const body = screen.getByTestId('body');
  body.scrollTop = 200;
  fireEvent.scroll(body);
  view.rerender(<Harness work={null} tab="overview" />);
  expect(body.scrollTop).toBe(0);
});
