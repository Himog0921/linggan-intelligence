import { fireEvent, render, screen, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { newDocument, type CanvasBlock, type Excerpt, type WorkDocument } from '../workbenchModel';
import * as api from './assetsApi';
import * as stylesApi from './stylesApi';
import type { AccountView, Experience, FactCardView, Identity, PersonaFact, Sentence } from './assetsModel';
import { MaterialsTab } from './MaterialsTab';

vi.mock('./assetsApi', async importOriginal => ({ ...await importOriginal<typeof import('./assetsApi')>(), listExperiences: vi.fn(), listPersonaFacts: vi.fn(), listFactCards: vi.fn() }));
vi.mock('./stylesApi', async importOriginal => ({ ...await importOriginal<typeof import('./stylesApi')>(), listStyles: vi.fn() }));

const ACCOUNT = '00000000-0000-4000-8000-000000000012';
const DOMAIN = '00000000-0000-4000-8000-000000000011';
const identity = (patch: Partial<Identity> = {}): Identity => ({
  accountRef: ACCOUNT, revision: 1, orgRelation: 'parent', bioDisclosesRelation: true, bioCheckedAt: 't', bioCheckedBy: 'local_machine',
  hasRealPerson: true, backingPersonRole: '妈妈', canProvideExperience: null, savedAt: 't', savedBy: 'local_machine', ...patch,
});
const account = (patch: Partial<AccountView> = {}): AccountView => ({ accountRef: ACCOUNT, accountName: '示例账号', platform: 'xhs', status: 'active', operatorName: null, identity: identity(), ...patch });
const sentence = (code: string, text: string): Sentence => ({
  code, revision: 1, group: 'parent', text, status: 'ok', statusNote: null, origin: 'manual', entryFindings: [], usedCount: 0, savedAt: 't', savedBy: 'local_machine',
});
const experience = (patch: Partial<Experience> = {}): Experience => ({
  id: '00000000-0000-4000-8000-0000000000e1', code: 'JL-0001', accountRef: ACCOUNT, currentPerson: true, state: 'confirmed', revision: 1,
  title: '一次小事', body: '那天我们慢慢来。', tags: null, origin: null, sendToModel: null, confirmedAt: 't', confirmedSubjectRole: '妈妈', ...patch,
});
const fact = (patch: Partial<PersonaFact> = {}): PersonaFact => ({
  id: '00000000-0000-4000-8000-0000000000f1', accountRef: ACCOUNT, key: '年级', version: 1, currentRevision: 1, currentPerson: true, state: 'active',
  text: '孩子现在三年级', source: 'manual', status: 'confirmed', confirmationMethod: null, ...patch,
});
const citation = { asOf: '2026-11-01T00:00:00Z', blockRef: 'b', sourceKind: 'note' as const, sourceRef: 's', workRef: null, field: 'body' as const, sourceVersion: 'v', start: 0, end: 4, textHash: 'h', location: null };
const excerpt = (id: string, selectedText: string | null, sourceKind: 'note' | 'comment' = 'note'): Excerpt => ({ id, citation: { ...citation, sourceKind }, selectedText, textHash: 'h' });
const block = (id: string, excerptId: string | null, patch: Partial<CanvasBlock> = {}): CanvasBlock => ({ id, excerptId, group: 'opening', note: '', expression: '', expressionRevision: 0, ...patch });

const doc = (patch: Partial<WorkDocument> = {}): WorkDocument => ({ ...newDocument('合成标题'), accountRef: ACCOUNT, ...patch });
const factCard = (): FactCardView => {
  const card = { code: 'CP-0001', revision: 1, name: '示例产品', allowedNames: ['示例营', '示例课'], bannedNames: ['旧叫法'], what: '', who: '', canSay: '', cannotSay: '不承诺效果', mustNote: '', confirmedBy: 'x', confirmedAt: 't', savedAt: 't', savedBy: 'local_machine' };
  return { code: 'CP-0001', latest: card, confirmed: card };
};

type Props = Partial<Parameters<typeof MaterialsTab>[0]>;
function show(props: Props = {}) {
  const handlers = { onUseBridge: vi.fn(), onInsertExperience: vi.fn(), onPlaceCard: vi.fn(), onGoDesk: vi.fn(), onOpenAssets: vi.fn(), onOpenSource: vi.fn() };
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}>
    <MaterialsTab document={doc()} domainRef={DOMAIN} account={account()} bridgeState="ready" bridgeItems={[]} enabled locked={false} {...handlers} {...props} />
  </QueryClientProvider>);
  return handlers;
}

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(api.listExperiences).mockResolvedValue([]);
  vi.mocked(api.listPersonaFacts).mockResolvedValue([]);
  vi.mocked(api.listFactCards).mockResolvedValue([]);
  vi.mocked(stylesApi.listStyles).mockResolvedValue([]);
});

describe('desk cards of this draft', () => {
  const withCards = () => doc({
    ideaRef: '00000000-0000-4000-8000-0000000000a1', ideaName: '作业启动', angle: '把第一步变小',
    excerpts: [excerpt('e1', '孩子一坐下就开始发呆', 'note'), excerpt('e2', '我家也是', 'comment'), excerpt('e3', null)],
    canvas: [block('c1', 'e1', { note: '当开头' }), block('c2', 'e2', { group: 'inbox' }), block('c3', 'e3'), block('c4', null, { expression: '我自己的想法', group: 'argument' })],
  });
  it('shows the idea box, its intent and every card with its role, kind, text and memo', () => {
    show({ document: withCards() });
    expect(screen.getByText('选题框', { exact: false })).toHaveTextContent('作业启动');
    expect(screen.getByText('把第一步变小')).toBeVisible();
    const rows = document.querySelectorAll('.mat-row');
    expect(rows).toHaveLength(4);
    expect(rows[0]).toHaveTextContent('开头');
    expect(rows[0]).toHaveTextContent('原笔记');
    expect(rows[0]).toHaveTextContent('孩子一坐下就开始发呆');
    expect(rows[0]).toHaveTextContent('思路：当开头');
    expect(rows[1]?.querySelector('.role-chip')).toHaveClass('soft');
    expect(rows[1]).toHaveTextContent('评论');
    expect(rows[3]).toHaveTextContent('便签');
    expect(rows[3]).toHaveTextContent('我自己的想法');
  });
  it('places a card through the placement dialog, marks used cards, and never offers a restricted source', () => {
    const base = withCards();
    const handlers = show({ document: { ...base, uses: [{ id: 'u1', canvasId: 'c2', expressionRevision: 0, excerptId: 'e2', target: { kind: 'paragraph', paragraphId: 'p' }, placedTextHash: 'h', state: 'current', contentOrigin: 'excerpt', sourceSpan: null }] } });
    const rows = [...document.querySelectorAll<HTMLElement>('.mat-row')];
    fireEvent.click(within(rows[0]!).getByRole('button', { name: /插入正文/ }));
    expect(handlers.onPlaceCard).toHaveBeenCalledWith('c1');
    expect(within(rows[1]!).getByText('本篇已用')).toBeVisible();
    expect(within(rows[1]!).queryByRole('button', { name: /插入正文/ })).toBeNull();
    expect(rows[2]).toHaveTextContent('来源已受限或不可用');
    expect(within(rows[2]!).queryByRole('button', { name: /插入正文/ })).toBeNull();
  });
  it('goes back to the original and to the desk', () => {
    const handlers = show({ document: withCards() });
    fireEvent.click(within(document.querySelector<HTMLElement>('.mat-row')!).getByRole('button', { name: '回到原文 ↗' }));
    expect(handlers.onOpenSource).toHaveBeenCalledWith(expect.objectContaining({ sourceKind: 'note' }));
    fireEvent.click(screen.getByRole('button', { name: '去画布整理' }));
    expect(handlers.onGoDesk).toHaveBeenCalledOnce();
  });
  it('says so when there is no idea box, intent or card, and disables insertion while the draft is locked', () => {
    show({ document: doc() });
    expect(screen.getByText('还没写“这篇想说什么”。')).toBeVisible();
    expect(screen.getByText(/还没有关联选题框/)).toBeVisible();
    expect(screen.getByText('还没有材料。可以去画布找材料，放进这一篇。')).toBeVisible();
  });
  it('does not offer insertion while locked', () => {
    show({ document: withCards(), locked: true });
    expect(screen.queryByRole('button', { name: /插入正文/ })).toBeNull();
  });
});

describe('experiences of the account', () => {
  it('lists confirmed and waiting experiences, inserts only confirmed unused ones, and marks the ones already in the draft', async () => {
    const carried = experience({ id: '00000000-0000-4000-8000-0000000000e2', code: 'JL-0002', title: '已经用过', revision: 4 });
    vi.mocked(api.listExperiences).mockResolvedValue([experience({ usedCount: 2 }), carried, experience({ id: 'w', code: 'JL-0003', title: '等本人确认', state: 'awaiting_subject' }), experience({ id: 'g', code: 'JL-0004', state: 'withdrawn', body: null })]);
    const handlers = show({ document: doc({ experienceSlots: [{ paragraphId: 'p', experienceRef: carried.id, experienceRevision: 4 }] }) });
    expect(await screen.findByText('一次小事', { exact: false })).toBeVisible();
    expect(screen.getByText('2 篇已保存稿件引用')).toBeVisible();
    expect(screen.getByText('待本人确认')).toBeVisible();
    expect(screen.queryByText('JL-0004')).toBeNull();
    const rows = [...document.querySelectorAll<HTMLElement>('.exp-mini')];
    expect(rows).toHaveLength(3);
    expect(within(rows[1]!).getByText('本篇已用')).toBeVisible();
    expect(within(rows[2]!).queryByRole('button', { name: /插入经历/ })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: '插入经历 JL-0001' }));
    expect(handlers.onInsertExperience).toHaveBeenCalledWith(expect.objectContaining({ code: 'JL-0001' }));
    fireEvent.click(screen.getByRole('button', { name: '去采集' }));
    expect(handlers.onOpenAssets).toHaveBeenCalledWith('experience');
  });
  it('does not ask for first-person experiences when nobody is behind the account', async () => {
    show({ account: account({ identity: identity({ hasRealPerson: false }) }) });
    expect(await screen.findByText('这个账号背后没有真人，不用第一人称经历。')).toBeVisible();
    expect(api.listExperiences).toHaveBeenCalled();
  });
  it('asks for an account first and reads nothing', () => {
    show({ document: doc({ accountRef: null }), account: null });
    const section = screen.getByRole('heading', { name: /经历 · 这个账号/ }).closest('.r-sec') as HTMLElement;
    expect(within(section).getByText('先为这篇选择账号。')).toBeVisible();
    expect(api.listExperiences).not.toHaveBeenCalled();
  });
  it('reports a failed read and retries on request', async () => {
    vi.mocked(api.listExperiences).mockRejectedValueOnce(new Error('boom')).mockResolvedValueOnce([experience()]);
    show();
    expect(await screen.findByText('经历读取失败。')).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: '重试' }));
    expect(await screen.findByText('一次小事', { exact: false })).toBeVisible();
  });
});

describe('bridge sentences', () => {
  it('lists the approved sentences, marks the one in use, and picks another on request', () => {
    const handlers = show({ bridgeItems: [sentence('XJ-0001', '我也是学员家长'), sentence('XJ-0002', '一起慢慢来')], document: doc({ bridge: { skip: false, afterParagraph: null, sentence: { sentenceCode: 'XJ-0001', sentenceRevision: 1, group: 'parent', text: '我也是学员家长', edited: false } } }) });
    expect(screen.getByText('衔接句 · 陪跑营学员家长')).toBeVisible();
    expect(screen.getByText('在用')).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: /用这句：一起慢慢来/ }));
    expect(handlers.onUseBridge).toHaveBeenCalledWith(expect.objectContaining({ code: 'XJ-0002' }));
  });
  it('explains why there is no list when the account cannot carry a bridge, and when reading failed', () => {
    show({ bridgeState: 'bio_missing', bridgeItems: null });
    expect(screen.getByText('账号简介还没写明和机构的关系，不放衔接句。')).toBeVisible();
  });
  it('reports a failed read instead of an empty list', () => {
    show({ bridgeItems: null, bridgeFailed: true });
    expect(screen.getByText('衔接句读取失败。')).toBeVisible();
    expect(screen.queryByText(/还没有审核通过的衔接句/)).toBeNull();
  });
  it('says that nothing is approved yet when the list is genuinely empty', () => {
    show({ bridgeItems: [] });
    expect(screen.getByText(/还没有审核通过的衔接句/)).toBeVisible();
  });
});

describe('reference assets', () => {
  it('folds persona facts with their status, product card, writing steps and the account style', async () => {
    vi.mocked(api.listPersonaFacts).mockResolvedValue([fact(), fact({ id: 'f2', key: '确诊时间', text: null, status: 'missing' }), fact({ id: 'f3', text: '推断同校', status: 'inferred' }), fact({ id: 'f4', text: '旧真人', currentPerson: false }), fact({ id: 'f5', text: '已撤回', state: 'withdrawn' })]);
    vi.mocked(api.listFactCards).mockResolvedValue([factCard()]);
    vi.mocked(stylesApi.listStyles).mockResolvedValue([{ code: 'FG-0001', accountRef: ACCOUNT, revision: 1, name: '她本人的原声', sourceNote: '', structure: [], rhythm: ['短句'], wording: ['口语'], skip: [], savedAt: 't' }]);
    const handlers = show({ document: doc({ paragraphSteps: [{ paragraphId: 'p', step: 'opening' }, { paragraphId: 'q', step: 'ending' }] }) });
    expect(await screen.findByText('人设事实 · 3 条')).toBeVisible();
    expect(screen.getByText('孩子现在三年级')).toBeInTheDocument();
    expect(screen.getByText('确诊时间')).toBeInTheDocument();
    expect(screen.getByText('缺')).toBeInTheDocument();
    expect(screen.getByText('待确认')).toBeInTheDocument();
    expect(screen.queryByText('旧真人')).toBeNull();
    expect(screen.queryByText('已撤回')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: '去补充' }));
    expect(handlers.onOpenAssets).toHaveBeenCalledWith('persona');
    expect(screen.getByText('写法步骤 · 已用 2/7')).toBeVisible();
    expect([...document.querySelectorAll('.arc span.on')].map(node => node.textContent)).toEqual(['开头', '收尾']);
    expect(await screen.findByText('正文可用：示例营、示例课')).toBeInTheDocument();
    expect(screen.getByText('不能讲：不承诺效果')).toBeInTheDocument();
    expect(await screen.findByText('风格 · 她本人的原声')).toBeVisible();
    expect([...document.querySelectorAll('.trait li')].map(node => node.textContent)).toEqual(['短句', '口语']);
  });
  it('says the name check will not run when no product card is confirmed, and shows no style fold without one', async () => {
    vi.mocked(api.listFactCards).mockResolvedValue([{ ...factCard(), confirmed: null }]);
    show();
    expect(await screen.findByText('还没有已确认的产品事实卡，叫法检查不会运行。')).toBeInTheDocument();
    expect(screen.queryByText(/^风格 ·/)).toBeNull();
  });
});

describe('while the page may not show personal assets', () => {
  it('reads nothing and shows only a notice', () => {
    show({ enabled: false });
    expect(screen.getByText('正在核验稿件，经历和事实暂时不显示。')).toBeVisible();
    expect(api.listExperiences).not.toHaveBeenCalled();
    expect(api.listPersonaFacts).not.toHaveBeenCalled();
    expect(stylesApi.listStyles).not.toHaveBeenCalled();
  });
});
