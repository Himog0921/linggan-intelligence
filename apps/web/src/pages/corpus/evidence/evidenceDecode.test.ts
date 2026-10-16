import { describe, expect, it } from 'vitest';
import { ApiError } from '../../../shared/api';
import { parseCommentPage, parseWorkDetail, parseWorkList } from './evidenceDecode';
import { commentsJson, detailJson, listItemJson, listJson, mediaHandle, slotJson, workRef } from './evidenceFixtures';

function failure(run: () => unknown): ApiError {
  try {
    run();
  } catch (error) {
    if (error instanceof ApiError) return error;
    throw error;
  }
  throw new Error('expected a failure');
}

describe('列表解码（白名单＋运行时校验）', () => {
  it('解码真实形状：身份、展示、互动、通道、原声、观察时刻', () => {
    const list = parseWorkList(listJson([listItemJson(1, { role: 'reference' })], { cursor: 'next', truncated: true, scanLimited: true, scannedCount: 51 }));
    const item = list.items[0]!;
    expect(item).toMatchObject({ ref: workRef(1), platform: 'xhs', role: 'reference', lastObservedAt: '2026-10-10 17:00' });
    expect(item.display).toMatchObject({ title: '合成作品 1', titleState: 'KNOWN', creatorName: '合成作者 1', publishedAt: '2026-10-13 13:28', publishedAtState: 'KNOWN' });
    expect(item.display.engagement.like).toEqual({ value: 1201, state: 'KNOWN' });
    expect(item.lanes.find(lane => lane.lane === 'detail')).toMatchObject({ state: 'SEARCHABLE', valueState: 'KNOWN', retained: 1 });
    expect(item.fragment?.sourceKind).toBe('detail_body');
    expect(list).toMatchObject({ cursor: 'next', truncated: true, scanLimited: true, scannedCount: 51 });
  });

  it('未知保持未知：缺失的互动是 null＋UNKNOWN，不是 0；没有标题就是 null', () => {
    const item = parseWorkList(listJson([listItemJson(1, { likes: null, comments: null, title: null, author: null, published: null })])).items[0]!;
    expect(item.display.engagement.like).toEqual({ value: null, state: 'UNKNOWN' });
    expect(item.display.engagement.comment).toEqual({ value: null, state: 'UNKNOWN' });
    expect(item.display.title).toBeNull();
    expect(item.display.titleState).toBe('UNKNOWN');
    expect(item.display.creatorName).toBeNull();
    expect(item.display.publishedAtState).toBe('UNKNOWN');
  });

  it('原声：没有文字当作没有原声；命中偏移不是整数就丢弃', () => {
    expect(parseWorkList(listJson([listItemJson(1, { fragment: null })])).items[0]!.fragment).toBeNull();
    expect(parseWorkList(listJson([listItemJson(1, { fragment: { text: '', sourceKind: 'detail_body' } })])).items[0]!.fragment).toBeNull();
    const hit = parseWorkList(listJson([listItemJson(1, { fragment: { text: '合成关键词', sourceKind: 'comment_body', selectionBasis: 'SEARCH_MATCH', matchOffset: 2, matchLength: 3, truncated: true } })])).items[0]!.fragment!;
    expect(hit).toMatchObject({ matchOffset: 2, matchLength: 3, truncated: true, sourceKind: 'comment_body' });
    const odd = parseWorkList(listJson([listItemJson(1, { fragment: { text: 'x', matchOffset: 1.5, matchLength: null } })])).items[0]!.fragment!;
    expect(odd).toMatchObject({ matchOffset: null, matchLength: null });
  });

  it('媒体：只采用受控本地句柄；头像还要 INLINE_SAFE；统计与旧页同一算法', () => {
    const rich = parseWorkList(listJson([listItemJson(1, { media: { cover: 'available', images: 2, videos: 1, avatar: 'inline' } })])).items[0]!.media;
    expect(rich).toMatchObject({ coverState: 'AVAILABLE', imageCount: 2, coverCandidateCount: 1, videoCount: 1, state: 'OBSERVED' });
    expect(rich.coverUrl).toBe(mediaHandle(10));
    expect(rich.avatarUrl).toBe(mediaHandle(100));
    // 2 张图 + 1 个封面候选 + 1 个视频 = 4 个带句柄的对象，再加封面本身 1 个。
    expect(rich.localObjectCount).toBe(5);
    const blocked = parseWorkList(listJson([listItemJson(1, { media: { avatar: 'blocked' } })])).items[0]!.media;
    expect(blocked.avatarUrl).toBeNull();
    expect(blocked.avatarState).toBe('ACQUIRED');
    const none = parseWorkList(listJson([listItemJson(1)])).items[0]!.media;
    expect(none).toMatchObject({ coverUrl: null, avatarUrl: null, coverState: 'NOT_OBSERVED', imageCount: 0, localObjectCount: 0 });
  });

  it('远程或不安全的封面地址一律不采用', () => {
    const item = listItemJson(1, { media: { cover: 'available' } });
    item.media.cover.localAssetUrl = 'https://cdn.example/cover.jpg';
    expect(parseWorkList(listJson([item])).items[0]!.media.coverUrl).toBeNull();
  });

  it('必需字段缺失或类型不对：报「返回内容无法解析」，不拿残缺数据渲染', () => {
    const good = listJson([listItemJson(1)]) as unknown as Record<string, unknown>;
    for (const mutate of [
      (value: Record<string, unknown>) => delete value.items,
      (value: Record<string, unknown>) => (value.items = 'x'),
      (value: Record<string, unknown>) => ((value.items as Record<string, unknown>[])[0]!.identity = {}),
      (value: Record<string, unknown>) => delete (value.items as Record<string, unknown>[])[0]!.identity,
    ]) {
      const bad = structuredClone(good);
      mutate(bad);
      expect(failure(() => parseWorkList(bad)).failure).toEqual({ kind: 'invalid_response', status: 200 });
    }
    expect(failure(() => parseWorkList(null)).failure.kind).toBe('invalid_response');
  });

  it('不渲染的字段被丢弃：平台用户标识、账号镜头不会出现在解码结果里', () => {
    const item = listItemJson(1) as Record<string, unknown>;
    item.authorExternalId = 'platform-user-9';
    (item.collectionContext as Record<string, unknown>).platformUserId = 'u-1';
    const decoded = JSON.stringify(parseWorkList(listJson([item])));
    expect(decoded).not.toContain('platform-user-9');
    expect(decoded).not.toContain('u-1');
  });
});

describe('详情解码', () => {
  it('解码检查器与通道：正文、互动当前值与前值、时间线、覆盖、回执、来源血缘', () => {
    const detail = parseWorkDetail(detailJson(1, { body: '完整正文', slots: [slotJson(1, workRef(1), { purpose: 'cover', ordinal: 1 })], limitations: ['RAW_BODY_NOT_RETURNED'] }));
    expect(detail.item.ref).toBe(workRef(1));
    expect(detail.inspector.detail.body).toMatchObject({ state: 'KNOWN', value: '完整正文' });
    expect(detail.inspector.detail.body?.sourceRef).not.toBeNull();
    expect(detail.inspector.engagement.likeCount).toMatchObject({ state: 'KNOWN', value: 1201, previous: 1196, delta: 5, deltaState: 'KNOWN' });
    expect(detail.inspector.engagement.commentCount).toMatchObject({ value: 8, previous: null, delta: null, deltaState: 'UNKNOWN' });
    expect(detail.inspector.timeline).toHaveLength(1);
    expect(detail.inspector.timeline[0]!.like).toEqual({ value: 1201, state: 'KNOWN' });
    expect(detail.inspector.commentsCoverage).toMatchObject({ count: 45, countState: 'KNOWN', retained: 45 });
    expect(detail.inspector.commentsReceipt).toEqual({ total: 48, returned: 0, truncated: true, nextCursor: null });
    expect(detail.inspector.limitations).toEqual(['RAW_BODY_NOT_RETURNED']);
    expect(detail.inspector.provenance?.stationAccountLensState).toBe('UNKNOWN');
    expect(detail.inspector.provenance?.packageRefs).toHaveLength(1);
    expect(detail.channels.commentsUrl).toContain('/comments');
    expect(detail.channels.reobservation).toMatchObject({ supported: true, eligible: false, url: null, reason: 'TARGET_LINKED_ACTIVE_DEEP_ARCHIVE_AUTHORIZATION_REQUIRED' });
    expect(detail.inspector.mediaSlots[0]).toMatchObject({ purpose: 'cover', assetUrl: mediaHandle(1), deliveryState: 'INLINE_SAFE', mime: 'image/png', hasHandle: true });
  });

  it('媒体槽位：有句柄但不是受控路径时不采用地址；没有句柄就是没有', () => {
    const remote = slotJson(1, workRef(1), { handle: 'https://cdn.example/x.jpg' });
    const none = slotJson(2, workRef(1), { handle: null });
    const slots = parseWorkDetail(detailJson(1, { slots: [remote, none] })).inspector.mediaSlots;
    expect(slots[0]).toMatchObject({ assetUrl: null, hasHandle: true });
    expect(slots[1]).toMatchObject({ assetUrl: null, hasHandle: false });
  });

  it('可复观测入口：缺省是「未知资格」；有入口地址时原样保留供上层校验', () => {
    const bare = parseWorkDetail({ item: detailJson(1).item });
    expect(bare.channels.reobservation).toMatchObject({ supported: false, eligible: false, url: null, reason: 'REOBSERVATION_ELIGIBILITY_UNKNOWN' });
    const eligible = parseWorkDetail(detailJson(1, { reobservation: { available: true, eligible: true, url: `/api/local/work-resources/${workRef(1)}/reobserve?domain=d` } }));
    expect(eligible.channels.reobservation).toMatchObject({ eligible: true, url: `/api/local/work-resources/${workRef(1)}/reobserve?domain=d` });
  });

  it('没有检查器也不崩：全部按未知处理；缺 item 才报无法解析', () => {
    const minimal = parseWorkDetail({ item: { identity: { publicRef: workRef(1), platform: 'xhs' } } });
    expect(minimal.inspector.timeline).toEqual([]);
    expect(minimal.inspector.detail.body).toBeNull();
    expect(minimal.inspector.mediaSlots).toEqual([]);
    expect(failure(() => parseWorkDetail({ channels: {} })).failure.kind).toBe('invalid_response');
  });
});

describe('评论页解码', () => {
  it('匿名评论与回复、总数、游标；只收白名单字段', () => {
    const page = parseCommentPage(commentsJson(20, { cursor: 'c2', total: 48 }));
    expect(page.items).toHaveLength(20);
    expect(page.items.some(comment => comment.relation === 'REPLY')).toBe(true);
    expect(page).toMatchObject({ total: 48, returned: 20, truncated: true, nextCursor: 'c2' });
    expect(JSON.stringify(page)).not.toContain('anonymousAuthorContext');
    expect(JSON.stringify(page)).not.toContain('WITHHELD');
  });

  it('正文未知时 body 可以是 null；缺来源引用报无法解析', () => {
    const json = commentsJson(1) as unknown as { items: Record<string, unknown>[] };
    json.items[0]!.body = null;
    json.items[0]!.bodyState = 'UNKNOWN';
    expect(parseCommentPage(json).items[0]).toMatchObject({ body: null, bodyState: 'UNKNOWN' });
    delete json.items[0]!.sourceRef;
    expect(failure(() => parseCommentPage(json)).failure.kind).toBe('invalid_response');
  });
});
