import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '../../../shared/api';
import { relayerDossier, fetchDossier, fetchDossierComments, fetchCreatorPreview, fetchRelayerPlan } from './dossierApi';
import { parseCreatorPreview, parseDossierComments, parseDossierPayload, parseRelayerPlanResponse, parseRelayerReceipt } from './dossierDecode';
import { comparableImages, exportFileName, fieldName, imageSummary, lineRole, nestComments, processingState, sectionEmptyNote, taskNote } from './dossierPresent';
import { creatorPreviewJson, dossierCommentsJson, dossierJson, dossierMediaJson, mediaHandle, planJson, relayerReceiptJson, workRef, DOMAIN } from './evidenceFixtures';

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('底稿解码', () => {
  it('解码片段、栏目、完整度、元数据、重新分层计划和三份任务材料', () => {
    const payload = parseDossierPayload(dossierJson(1));
    expect(payload.dossier.fragments.map(fragment => fragment.field)).toEqual(['title', 'body']);
    expect(payload.dossier.fragments[1]?.end).toBe(600);
    expect(payload.dossier.sections.map(section => section.kind)).toEqual(['title', 'cover_headline', 'body', 'comments']);
    expect(payload.dossier.coverage).toMatchObject({ title: 'KNOWN', body: 'KNOWN', mediaAvailable: 1, mediaCount: 8, uncertainImages: 1, titleQualification: 'source_value' });
    expect(payload.dossier.metadata).toMatchObject({ platform: 'xhs', authorDisplayName: '合成作者', publishedAt: '2026-10-13 13:28' });
    expect(payload.dossier.metadata.likes).toEqual({ value: 1200, state: 'KNOWN' });
    expect(payload.relayerPlan).toMatchObject({ layoutCount: 1, pendingLayouts: 1, needsOcr: 3, expectedRevision: 'r'.repeat(64) });
    expect(Object.keys(payload.inputs)).toEqual(['creator', 'comments', 'topics']);
    expect(payload.inputs.comments.fragments[0]).toMatchObject({ field: 'native_title', characterCount: 6 });
    expect(payload.inputs.creator.markdown).toContain('平台标题');
    expect(payload.inputs.topics.truncated).toBe(true);
    expect(payload.markdown).toContain('合成底稿');
  });

  it('原图句柄只采用 /api/local/media/；识别行坐标必须在 0–1，否则不画框', () => {
    const media = parseDossierPayload(dossierJson(1, { media: [dossierMediaJson(1), dossierMediaJson(2, { handle: 'https://cdn.example/x.jpg' }), dossierMediaJson(3, { handle: '/api/local/derivative/x' })] })).dossier.media;
    expect(media[0]?.originalUrl).toBe(mediaHandle(1));
    expect(media[1]?.originalUrl).toBeNull();
    expect(media[2]?.originalUrl).toBeNull();
    const layered = dossierMediaJson(1) as { ocrLayering: { lines: { bbox: number[] }[] } };
    layered.ocrLayering.lines[0]!.bbox = [0.1, 0.2, 1.4, 0.3];
    layered.ocrLayering.lines[1]!.bbox = [0.1, 0.2, 0.3] as unknown as number[];
    const lines = parseDossierPayload(dossierJson(1, { media: [layered] })).dossier.media[0]!.layering!.lines;
    expect(lines.map(line => line.bbox)).toEqual([null, null, [0, 0.9, 0.2, 0.92]]);
  });

  it('分层：保留行与已排除行各按行引用归类', () => {
    const layering = parseDossierPayload(dossierJson(1)).dossier.media[0]!.layering!;
    expect(Object.values(layering.retained)).toEqual(['substantive_text', 'primary_copy']);
    expect(Object.values(layering.excluded)).toEqual(['platform_watermark']);
    expect(layering).toMatchObject({ layerVersion: 'rules-v1', state: 'PARTIAL', imageWidth: 240 });
  });

  it('导出用的原样底稿保留在 raw 里（含平台作者编号，旧页如此，DR-9），白名单字段里没有', () => {
    const payload = parseDossierPayload(dossierJson(1));
    expect(JSON.stringify(payload.dossier.metadata)).not.toContain('synthetic-author-id');
    expect(JSON.stringify(payload.dossier.raw)).toContain('synthetic-author-id');
  });

  it('必需字段缺失：报无法解析；计划缺修订报无法解析', () => {
    for (const bad of [{}, { dossier: {} }, { ...dossierJson(1), relayerPlan: { layoutCount: 1 } }]) {
      expect(() => parseDossierPayload(bad)).toThrow(ApiError);
    }
    expect(parseRelayerPlanResponse({ relayerPlan: planJson({ pendingLayouts: 0, alreadyCurrent: 1 }) })).toMatchObject({ pendingLayouts: 0, alreadyCurrent: 1 });
  });

  it('完整请求预览：可用与不可用两种；原样 raw 只作折叠诊断', () => {
    expect(parseCreatorPreview(creatorPreviewJson())).toMatchObject({ available: true, inputTokenLimit: 8192, estimatedInputTokensUpperBound: 6195, materials: expect.stringContaining('平台标题') });
    expect(parseCreatorPreview({ available: false, reason: 'model_not_configured' })).toMatchObject({ available: false, reason: 'model_not_configured', materials: null });
  });

  it('底稿评论与重新分层回执', () => {
    const page = parseDossierComments(dossierCommentsJson(30, { cursor: 'c', total: 48 }));
    expect(page).toMatchObject({ total: 48, nextCursor: 'c' });
    expect(page.items[0]).toMatchObject({ relation: 'ROOT', likeCount: 5 });
    expect(JSON.stringify(page)).not.toContain('WITHHELD');
    expect(parseRelayerReceipt(relayerReceiptJson({ needsOcr: 3 }))).toEqual({ scopeCount: 1, updated: 1, alreadyCurrent: 0, skipped: 0, needsOcr: 3 });
    expect(() => parseRelayerReceipt({})).toThrow(ApiError);
  });
});

describe('底稿的词表与整理', () => {
  it('栏目名、处理状态、识别行角色、完整度词', () => {
    expect(fieldName('cover_headline')).toBe('图片大字');
    expect(fieldName('mystery')).toBe('材料文字');
    expect(processingState('NEEDS_REVIEW')).toBe('待确认');
    expect(processingState('RETIRED')).toBe('旧识别已停用');
    expect(lineRole('platform_watermark')).toBe('平台水印');
    expect(lineRole('mystery')).toBe('待确认');
    expect(sectionEmptyNote('comments')).toBe('评论在下方单独读取。');
    expect(sectionEmptyNote('tags')).toContain('尚无独立标签记录');
    expect(sectionEmptyNote('title')).toContain('是否原本存在尚未确认');
    expect(taskNote('creator')).toContain('4,000');
    expect(taskNote('comments')).toContain('20,000');
    expect(exportFileName(workRef(1), 'md')).toBe(`研究底稿-${workRef(1)}.md`);
  });

  it('原图对照只列非仅上下文、且已分层或属于识图处理器的图', () => {
    const media = parseDossierPayload(dossierJson(1, { media: [dossierMediaJson(1), dossierMediaJson(2, { contextOnly: true }), dossierMediaJson(3, { layered: false })] })).dossier.media;
    expect(comparableImages(media).map(item => item.slotOrdinal)).toEqual([1, 3]);
    expect(imageSummary(media[0]!)).toBe('图片 1 · 部分待确认 · rules-v1');
    expect(imageSummary(media[2]!)).toBe('图片 3 · 已取得 · 尚未分层');
  });

  it('评论整理：回复挂在主评论下；父评论没读到的回复单独标出，不拼接其内容', () => {
    const items = [
      { sourceRef: 'a', parentSourceRef: null, relation: 'ROOT' as const },
      { sourceRef: 'b', parentSourceRef: 'a', relation: 'REPLY' as const },
      { sourceRef: 'c', parentSourceRef: 'zzz', relation: 'REPLY' as const },
      { sourceRef: 'd', parentSourceRef: 'b', relation: 'REPLY' as const },
    ];
    const nodes = nestComments(items);
    expect(nodes.map(node => node.item.sourceRef)).toEqual(['a', 'c', 'd']);
    expect(nodes[0]?.replies.map(reply => reply.sourceRef)).toEqual(['b']);
    expect(nodes[1]).toMatchObject({ orphan: true });
    expect(nodes[2]).toMatchObject({ orphan: false });
  });
});

describe('底稿请求', () => {
  function respond(body: unknown, status = 200) {
    const fetchMock = vi.fn<typeof fetch>(async () => new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } }));
    vi.stubGlobal('fetch', fetchMock);
    return fetchMock;
  }

  it('读接口都带领域；评论带游标', async () => {
    const mock = respond(dossierJson(1));
    await fetchDossier(workRef(1), DOMAIN);
    expect(mock.mock.calls[0]?.[0]).toBe(`/api/local/work-resources/${workRef(1)}/research-dossier?domain=${DOMAIN}`);
    respond({ relayerPlan: planJson() });
    await fetchRelayerPlan(workRef(1), DOMAIN);
    respond(creatorPreviewJson());
    await fetchCreatorPreview(workRef(1), DOMAIN);
    const comments = respond(dossierCommentsJson(1));
    await fetchDossierComments(workRef(1), DOMAIN, 'cur sor');
    expect(comments.mock.calls[0]?.[0]).toBe(`/api/local/work-resources/${workRef(1)}/research-dossier/comments?domain=${DOMAIN}&cursor=cur%20sor`);
  });

  it('重新分层：POST 体只有 expectedRevision 与 domain，且不自动重试', async () => {
    const mock = respond(relayerReceiptJson());
    await relayerDossier(workRef(1), DOMAIN, 'rev-1');
    expect(mock.mock.calls[0]?.[0]).toBe(`/api/local/work-resources/${workRef(1)}/research-dossier/relayer`);
    expect(mock.mock.calls[0]?.[1]).toMatchObject({ method: 'POST', body: JSON.stringify({ expectedRevision: 'rev-1', domain: DOMAIN }) });
    const failing = respond({ code: 'dossier_changed_or_scope_unavailable' }, 409);
    await expect(relayerDossier(workRef(1), DOMAIN, 'rev-1')).rejects.toMatchObject({ failure: { kind: 'http', status: 409, code: 'dossier_changed_or_scope_unavailable' } });
    expect(failing).toHaveBeenCalledTimes(1);
  });
});
