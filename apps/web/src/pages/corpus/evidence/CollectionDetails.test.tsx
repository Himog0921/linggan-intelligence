import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { CollectionDetails } from './CollectionDetails';
import { parseWorkDetail } from './evidenceDecode';
import { detailJson, mediaRef, slotJson, workRef, type DetailOptions } from './evidenceFixtures';
import type { PanelContext } from './ReadingPanel';

function context(options: DetailOptions = {}): PanelContext {
  const parsed = parseWorkDetail(detailJson(1, options));
  return {
    item: parsed.item,
    inspector: parsed.inspector,
    channels: parsed.channels,
    listItem: null,
    comments: {
      phase: 'idle',
      items: [],
      cursor: null,
      total: null,
      returned: null,
      truncated: false,
      error: null,
      load: () => undefined,
    },
    onOpenViewer: () => undefined,
  };
}

describe('采集详情（Mog 2026-11-02）', () => {
  it('默认收起；展开只保留来源、时间、材料、停止原因与必要编号', async () => {
    const view = render(<CollectionDetails context={context({
      sourceNoteUrl: 'https://www.xiaohongshu.com/explore/0123456789abcdef01234567',
      commentsCoverage: {
        count: 18,
        countState: 'KNOWN',
        retained: 18,
        uniqueCollectedCount: 18,
        stoppedReason: 'risk_control',
        collectionState: 'partial',
      },
      commentsHistory: [{
        observedAt: '2026-10-14T21:59:48Z',
        state: 'PARTIAL',
        collectionScope: 'all_public_comments',
        requestedLimit: null,
        pageCommentCount: 37,
        expectedCount: null,
        uniqueCollectedCount: 18,
        collectionState: 'partial',
        producerAcquired: 18,
        retained: 18,
        failed: 0,
        taskRef: null,
        attemptRef: null,
        packageRef: null,
        receiptRef: null,
      }],
      limitations: ['RAW_BODY_NOT_RETURNED'],
      slots: [slotJson(1, workRef(1))],
    })} />);
    const details = view.container.querySelector('details')!;
    expect(details).not.toHaveAttribute('open');
    expect(screen.getByText('采集详情')).toBeInTheDocument();
    expect(view.container).not.toHaveTextContent('材料完整度');
    expect(view.container).not.toHaveTextContent('互动数据观察时间线');
    expect(view.container).not.toHaveTextContent('当前没有本地媒体对象');
    expect(view.container).not.toHaveTextContent('合成作品 1');
    expect(view.container).not.toHaveTextContent('合成作者 1');
    expect(view.container).toHaveTextContent('平台风险控制导致停止');
    expect(view.container).toHaveTextContent('正文原文不在本次读取范围内');
    expect(view.container).toHaveTextContent('01 / 01 正文图片');
    expect(view.container).toHaveTextContent(mediaRef(501));

    await userEvent.setup().click(screen.getByText('采集详情'));
    expect(details).toHaveAttribute('open');
    expect(screen.getByRole('link', { name: '打开原文' })).toHaveAttribute('href', 'https://www.xiaohongshu.com/explore/0123456789abcdef01234567');
    expect(view.container).toHaveTextContent('2026-10-15 05:59');
  });

  it('没有本地媒体只说明事实，不铺故障警示', () => {
    const view = render(<CollectionDetails context={context()} />);
    expect(view.container).toHaveTextContent('媒体槽位返回 0，没有本地副本');
    expect(view.container).not.toHaveTextContent('当前没有本地媒体对象');
    expect(view.container).not.toHaveTextContent('当前没有可预览的本地媒体对象');
  });

  it('材料状态沿通道原词表达：KNOWN_EMPTY 不算可读，未知不推缺口', () => {
    const timelinePoint = {
      collectCount: 30,
      collectCountState: 'KNOWN',
      commentCount: 8,
      commentCountState: 'KNOWN',
      likeCount: 1201,
      likeCountState: 'KNOWN',
      observedAt: null,
      shareCount: 2,
      shareCountState: 'KNOWN',
      sourceLane: 'detail',
    };
    const view = render(<CollectionDetails context={context({
      lanes: { detail: 'KNOWN_EMPTY', comments: 'UNKNOWN' },
      observedAt: null,
      timeline: Array.from({ length: 6 }, () => ({ ...timelinePoint })),
      mediaSlotsReceipt: null,
      mediaReceipt: null,
    })} />);
    expect(view.container).toHaveTextContent('正文：已处理，未识别到内容');
    expect(view.container).toHaveTextContent('评论：当前未知');
    expect(view.container).not.toHaveTextContent('可读 1/5');
    expect(view.container).not.toHaveTextContent('有缺口');
    expect(view.container).toHaveTextContent('时间未知');
    expect(view.container).toHaveTextContent('本次未返回媒体槽位，无法判断本地是否有副本');
  });

  it('来源或限制未返回时保持未知，不推断为空', () => {
    const view = render(<CollectionDetails context={context({
      provenance: null,
      limitations: [],
      commentsCoverage: null,
    })} />);
    expect(view.container).toHaveTextContent('本次详情未返回来源血缘');
    expect(view.container).toHaveTextContent('本次读取未返回限制列表');
    expect(view.container).toHaveTextContent('本次采集未记录停止原因');
    expect(view.container).toHaveTextContent(workRef(1));
  });
});
