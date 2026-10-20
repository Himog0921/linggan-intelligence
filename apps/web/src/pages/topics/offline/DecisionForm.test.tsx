import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { DecisionForm } from './DecisionForm';
import type { Note, Preview } from './api';

// Small component props, deliberately distinct from the backend's authoritative sample fixture.
const identity = { domainRef: 'unit-domain', workRef: 'unit-work', snapshotIdentity: 'unit-snapshot', directionVersionRef: 'unit-directions', skillVersionRef: 'unit-skill' };
const machine: Note['baseline'] = { identity, relevance: 'related', primaryDirection: 'one', alsoRelated: null, reason: '初始合成理由', confidence: 'possible', perspective: 'unknown', keywords: [] };
const note: Note = { title: '组件单元示例', likes: null, publishedWeek: null, input: { identity, columns: [], fragments: [] }, baseline: machine, result: { machine, effective: machine, judgment: null, manualApplied: false, needsReview: false } };
const catalogue: Preview['catalogue'] = { versionRef: 'unit-directions', groups: [], directions: [{ directionRef: 'one', groupRef: 'group', name: '组件示例方向', definition: '组件单元测试', exampleWorkRefs: [] }] };
afterEach(cleanup);

it('fields have exact readable labels and clearing a reason submits null with the current identity', () => {
  const apply = vi.fn();
  render(<DecisionForm note={note} catalogue={catalogue} busy={false} apply={apply} reset={vi.fn()} />);
  expect(screen.getByLabelText('相关性', { exact: true })).toBeInTheDocument();
  expect(screen.getByLabelText('主方向', { exact: true })).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText('裁定理由', { exact: true }), { target: { value: '' } });
  fireEvent.click(screen.getByRole('button', { name: '应用到本次预演' }));
  expect(apply).toHaveBeenCalledWith(expect.objectContaining({ identity, reason: null }));
});

it('reason length counts Unicode characters rather than UTF-16 units', () => {
  render(<DecisionForm note={note} catalogue={catalogue} busy={false} apply={vi.fn()} reset={vi.fn()} />);
  const reason = screen.getByLabelText('裁定理由', { exact: true });
  fireEvent.change(reason, { target: { value: '🙂'.repeat(200) } });
  expect(screen.getByRole('button', { name: '应用到本次预演' })).toBeEnabled();
  fireEvent.change(reason, { target: { value: '🙂'.repeat(201) } });
  expect(screen.getByRole('button', { name: '应用到本次预演' })).toBeDisabled();
});
