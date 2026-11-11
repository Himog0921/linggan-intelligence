import { cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, expect, it } from 'vitest';
import { decodeSnapshot } from './topicDtos';
import { TopicPatterns } from './TopicPatterns';
import { topicSnapshot, topicWork } from './topicTestFixture';
import type { Work } from './types';
afterEach(cleanup);
function researched(index = 0): Work {
  return { ...topicWork(index), research: {
    readable: true, authorSourceState: 'available', commentSourceState: 'source_unavailable',
    fragments: [{ fragmentId: 'f1', start: 20, end: 24, text: '作者原句', field: 'core_text_evidence' }],
    output: { discussions: [], scenes: [], responseMatches: [], angles: [], productOpportunities: [], limitations: [] },
    contentPatterns: { coreRef: 'original-core', items: [
      { kind: 'content_type', label: 'personal_story', unitIds: ['u1'], evidence: [{ fragmentId: 'f1', start: 20, end: 24 }] },
      { kind: 'expression', label: '第一人称回顾', unitIds: ['u1'], evidence: [{ fragmentId: 'f1', start: 20, end: 24 }] },
    ] },
  } };
}
it('decodes original Core/unit identities and shows classified work counts separately from unknown', () => {
  const source = researched();
  const decoded = decodeSnapshot(topicSnapshot([source, topicWork(1)]));
  expect(decoded.works[0]!.research?.contentPatterns).toEqual(source.research?.contentPatterns);
  render(<TopicPatterns works={decoded.works} />);
  const types = screen.getByRole('region', { name: '内容类型' });
  expect(types).toHaveTextContent('已识别 1 / 2 篇 · 未知 1 篇');
  expect(within(types).getByRole('heading', { name: '亲历故事' })).toBeInTheDocument();
  expect(types.querySelector('[data-core-ref="original-core"][data-unit-ids="u1"]')).toHaveTextContent('作者原句');
  expect(within(types).getByRole('button', { name: '查看这些作品' })).toHaveAttribute('data-id', source.workRef);
  expect(screen.getByRole('region', { name: '回答形式' })).toHaveTextContent('已识别 0 / 2 篇 · 未知 2 篇');
});
it('counts a work once and does not infer a pattern from media shape, other, or missing citations', () => {
  const source = researched();
  source.research!.contentPatterns!.items.push(source.research!.contentPatterns!.items[1]!);
  const other = researched(1);
  other.research!.contentPatterns!.items = [{ kind: 'content_type', label: 'other', unitIds: ['u1'], evidence: [{ fragmentId: 'missing', start: 0, end: 1 }] }];
  other.media = { kind: 'video' };
  render(<TopicPatterns works={[source, other]} />);
  expect(screen.getByRole('region', { name: '表达方式' })).toHaveTextContent('已识别 1 / 2 篇 · 未知 1 篇');
  expect(screen.getByRole('region', { name: '内容类型' })).toHaveTextContent('已识别 1 / 2 篇 · 未知 1 篇');
  expect(screen.queryByRole('heading', { name: 'video' })).not.toBeInTheDocument();
});

it('keeps model-read image observations distinct from original text quotes', () => {
  const source = researched();
  source.research!.fragments[0]!.field = 'core_image_evidence';
  source.research!.fragments[0]!.text = '图示流程';
  source.research!.contentPatterns!.items = [source.research!.contentPatterns!.items[1]!];
  render(<TopicPatterns works={[source]} />);
  const expression = screen.getByRole('region', { name: '表达方式' });
  expect(expression).toHaveTextContent('图片理解依据');
  expect(expression).toHaveTextContent('图示流程');
  expect(expression.querySelector('blockquote')).toBeNull();
  expect(within(expression).queryByRole('button', { name: '核对原文' })).not.toBeInTheDocument();
  expect(within(expression).getByRole('button', { name: '查看来源' })).toHaveAttribute('data-id', source.workRef);
});
