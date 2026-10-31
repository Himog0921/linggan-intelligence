import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '../../../shared/api';
import * as api from './assetsApi';
import type { AccountView, FactCard, FactCardView, Identity, Sentence } from './assetsModel';
import { AssetsLibrary } from './AssetsLibrary';

vi.mock('./assetsApi', async importOriginal => {
  const original = await importOriginal<typeof import('./assetsApi')>();
  return {
    ...original, listAccounts: vi.fn(), putIdentity: vi.fn(), listFactCards: vi.fn(), saveFactCard: vi.fn(), confirmFactCard: vi.fn(),
    listSentences: vi.fn(), createSentence: vi.fn(), editSentence: vi.fn(), reviewSentence: vi.fn(), checkSentence: vi.fn(),
  };
});

const DOMAIN = '00000000-0000-4000-8000-000000000011';
const A1 = '00000000-0000-4000-8000-0000000000a1';
const A2 = '00000000-0000-4000-8000-0000000000a2';

const identity = (patch: Partial<Identity> = {}): Identity => ({
  accountRef: A1, revision: 2, orgRelation: 'parent', bioDisclosesRelation: null, bioCheckedAt: null, bioCheckedBy: null,
  hasRealPerson: null, backingPersonRole: null, canProvideExperience: null, savedAt: 't', savedBy: 'local_machine', ...patch,
});
const account = (ref: string, name: string, patch: Partial<AccountView> = {}): AccountView => ({
  accountRef: ref, accountName: name, platform: 'xhs', status: 'active', operatorName: null, identity: null, ...patch,
});
const card = (patch: Partial<FactCard> = {}): FactCard => ({
  code: 'CP-0001', revision: 1, name: '示例营', allowedNames: ['示例陪跑营'], bannedNames: ['示例课'], what: '', who: '', canSay: '', cannotSay: '',
  mustNote: '', confirmedBy: null, confirmedAt: null, savedAt: 't', savedBy: 'local_machine', ...patch,
});
const sentence = (patch: Partial<Sentence> = {}): Sentence => ({
  code: 'XJ-0001', revision: 1, group: 'parent', text: '我也是学员家长', status: 'review', statusNote: null, origin: 'manual',
  entryFindings: [], usedCount: 3, savedAt: 't', savedBy: 'local_machine', ...patch,
});

function show() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={client}><AssetsLibrary domainRef={DOMAIN} /></QueryClientProvider>);
}
const open = (name: string) => fireEvent.click(screen.getByRole('button', { name }));

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(api.listAccounts).mockResolvedValue([]);
  vi.mocked(api.listFactCards).mockResolvedValue([]);
  vi.mocked(api.listSentences).mockResolvedValue([]);
  vi.mocked(api.checkSentence).mockResolvedValue({ rulesetVersion: 'v', blocked: false, findings: [], notRun: [] });
});

describe('账号身份', () => {
  it('requires the author to distinguish a role correction from a replacement', async () => {
    const current = identity({ hasRealPerson: true, hasKnownPerson: true, backingPersonRole: '妈妈', bioDisclosesRelation: true });
    vi.mocked(api.listAccounts).mockResolvedValue([account(A1, '甲号', { identity: current })]);
    vi.mocked(api.putIdentity).mockResolvedValue({ identity: identity({ revision: 3, hasRealPerson: true, hasKnownPerson: true, backingPersonRole: '爸爸' }), replayed: false });
    show();
    fireEvent.click(await screen.findByRole('button', { name: /甲号/ }));
    const form = await screen.findByRole('form', { name: '甲号 · 账号身份' });
    fireEvent.change(within(form).getByLabelText(/真人的角色/), { target: { value: '爸爸' } });
    expect(within(form).getByRole('button', { name: '保存' })).toBeDisabled();
    fireEvent.click(within(form).getByLabelText('同一位真人，更正信息'));
    fireEvent.click(within(form).getByRole('button', { name: '保存' }));
    await waitFor(() => expect(api.putIdentity).toHaveBeenCalledWith(A1, expect.objectContaining({ personChange: 'correction', backingPersonRole: '爸爸' })));
  });

  it('can record replacement even when the role label stays the same', async () => {
    const current = identity({ hasRealPerson: true, hasKnownPerson: true, backingPersonRole: '妈妈', bioDisclosesRelation: true });
    vi.mocked(api.listAccounts).mockResolvedValue([account(A1, '甲号', { identity: current })]);
    vi.mocked(api.putIdentity).mockResolvedValue({ identity: identity({ revision: 3, hasRealPerson: true, hasKnownPerson: true, backingPersonRole: '妈妈' }), replayed: false });
    show();
    fireEvent.click(await screen.findByRole('button', { name: /甲号/ }));
    const form = await screen.findByRole('form', { name: '甲号 · 账号身份' });
    fireEvent.click(within(form).getByLabelText('更换了真人'));
    expect(within(form).getByText(/机构关系和简介状态都将设为未确认/)).toBeInTheDocument();
    expect(within(form).getByLabelText('还没确认')).toBeChecked();
    expect(within(form).getByLabelText('还没确认')).toBeDisabled();
    expect(within(form).queryByLabelText('已写明')).not.toBeInTheDocument();
    fireEvent.click(within(form).getByLabelText('同一位真人，更正信息'));
    expect(within(form).getByLabelText('已写明')).toBeChecked();
    fireEvent.click(within(form).getByLabelText('更换了真人'));
    fireEvent.click(within(form).getByRole('button', { name: '保存' }));
    await waitFor(() => expect(api.putIdentity).toHaveBeenCalledWith(A1, expect.objectContaining({ personChange: 'replacement', backingPersonRole: '妈妈', orgRelation: 'unknown', bioDisclosesRelation: null })));
  });

  it('lists accounts with what is known and never fills in an answer nobody gave', async () => {
    vi.mocked(api.listAccounts).mockResolvedValue([
      account(A1, '甲号', { identity: identity({ bioDisclosesRelation: true }), operatorName: '小王' }), account(A2, '乙号'),
    ]);
    show();
    expect(await screen.findByText('陪跑营学员家长 · 简介已写明 · 负责：小王')).toBeInTheDocument();
    expect(screen.getByText('还没填写')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /乙号/ }));
    const form = await screen.findByRole('form', { name: '乙号 · 账号身份' });
    expect(within(form).getByLabelText('还没确认')).toBeChecked();
    expect(within(form).queryByText('账号简介里写明这层关系了吗')).not.toBeInTheDocument();
    expect(within(form).getAllByLabelText('不清楚').every(box => (box as HTMLInputElement).checked)).toBe(true);
  });

  it('saves a new revision from the latest one and keeps unanswered questions unknown', async () => {
    vi.mocked(api.listAccounts).mockResolvedValue([account(A1, '甲号', { identity: identity() })]);
    vi.mocked(api.putIdentity).mockResolvedValue({ identity: identity({ revision: 3, bioDisclosesRelation: true }), replayed: false });
    show();
    fireEvent.click(await screen.findByRole('button', { name: /甲号/ }));
    const form = await screen.findByRole('form', { name: '甲号 · 账号身份' });
    fireEvent.click(within(form).getByLabelText('已写明'));
    fireEvent.click(within(form).getByLabelText('有'));
    fireEvent.change(within(form).getByLabelText(/真人的角色/), { target: { value: ' 妈妈 ' } });
    fireEvent.click(within(form).getByRole('button', { name: '保存' }));
    await waitFor(() => expect(api.putIdentity).toHaveBeenCalledTimes(1));
    expect(vi.mocked(api.putIdentity).mock.calls[0]?.[0]).toBe(A1);
    expect(vi.mocked(api.putIdentity).mock.calls[0]?.[1]).toMatchObject({
      expectedRevision: 2, orgRelation: 'parent', bioDisclosesRelation: true, hasRealPerson: true, backingPersonRole: '妈妈', canProvideExperience: null,
    });
  });

  it('does not keep a bio answer or a role that no longer applies', async () => {
    vi.mocked(api.listAccounts).mockResolvedValue([account(A1, '甲号', { identity: identity({ bioDisclosesRelation: true, hasRealPerson: true, backingPersonRole: '妈妈' }) })]);
    vi.mocked(api.putIdentity).mockResolvedValue({ identity: identity({ revision: 3, orgRelation: 'none' }), replayed: false });
    show();
    fireEvent.click(await screen.findByRole('button', { name: /甲号/ }));
    const form = await screen.findByRole('form', { name: '甲号 · 账号身份' });
    fireEvent.click(within(form).getByLabelText('和机构无关'));
    fireEvent.click(within(form).getAllByLabelText('没有')[0]!);
    fireEvent.click(within(form).getByRole('button', { name: '保存' }));
    await waitFor(() => expect(api.putIdentity).toHaveBeenCalledTimes(1));
    expect(vi.mocked(api.putIdentity).mock.calls[0]?.[1]).toMatchObject({ orgRelation: 'none', bioDisclosesRelation: null, hasRealPerson: false, backingPersonRole: null });
  });

  it('asks about the bio again when the relationship changes', async () => {
    vi.mocked(api.listAccounts).mockResolvedValue([account(A1, '甲号', { identity: identity({ bioDisclosesRelation: true }) })]);
    vi.mocked(api.putIdentity).mockResolvedValue({ identity: identity({ revision: 3, orgRelation: 'coach', bioDisclosesRelation: null }), replayed: false });
    show();
    fireEvent.click(await screen.findByRole('button', { name: /甲号/ }));
    const form = await screen.findByRole('form', { name: '甲号 · 账号身份' });
    expect(within(form).getByLabelText('已写明')).toBeChecked();
    fireEvent.click(within(form).getByLabelText('陪跑老师'));
    expect(within(form).getByLabelText('已写明')).not.toBeChecked();
    fireEvent.click(within(form).getByRole('button', { name: '保存' }));
    await waitFor(() => expect(api.putIdentity).toHaveBeenCalledWith(A1, expect.objectContaining({ orgRelation: 'coach', bioDisclosesRelation: null })));
  });

  it('says the revision is stale instead of overwriting, and re-reads', async () => {
    vi.mocked(api.listAccounts).mockResolvedValue([account(A1, '甲号', { identity: identity() })]);
    vi.mocked(api.putIdentity).mockRejectedValue(new ApiError({ kind: 'http', status: 409, code: 'creation_assets_stale' }));
    show();
    fireEvent.click(await screen.findByRole('button', { name: /甲号/ }));
    fireEvent.click(within(await screen.findByRole('form', { name: '甲号 · 账号身份' })).getByRole('button', { name: '保存' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('别处刚改过这里');
    await waitFor(() => expect(vi.mocked(api.listAccounts).mock.calls.length).toBeGreaterThan(1));
  });

  it('shows a failed read as a failure and an empty domain as empty', async () => {
    vi.mocked(api.listAccounts).mockRejectedValueOnce(new ApiError({ kind: 'network' }));
    show();
    expect(await screen.findByRole('alert')).toHaveTextContent('账号读取失败');
    vi.mocked(api.listAccounts).mockResolvedValue([]);
    fireEvent.click(screen.getByRole('button', { name: '重试' }));
    expect(await screen.findByText(/没有可写稿的小红书账号/)).toBeInTheDocument();
  });
});

describe('产品事实卡', () => {
  const views = (latest: FactCard, confirmed: FactCard | null): FactCardView[] => [{ code: latest.code, latest, confirmed }];

  it('shows which revision is in force and which one waits for confirmation', async () => {
    vi.mocked(api.listFactCards).mockResolvedValue(views(card({ revision: 2 }), card({ revision: 1, confirmedAt: 't' })));
    show();
    open('产品事实卡');
    expect(await screen.findByText('CP-0001 · 生效第 1 版 · 第 2 版待确认')).toBeInTheDocument();
  });

  it('creates a card from one-per-line names and confirms the latest revision only on request', async () => {
    vi.mocked(api.saveFactCard).mockResolvedValue({ card: card(), replayed: false });
    vi.mocked(api.confirmFactCard).mockResolvedValue({ card: card({ confirmedAt: 't', confirmedBy: 'local_machine' }), replayed: false });
    show();
    open('产品事实卡');
    fireEvent.click(await screen.findByRole('button', { name: '＋ 新建' }));
    const form = await screen.findByRole('form', { name: '新建产品事实卡' });
    expect(within(form).getByRole('button', { name: '保存' })).toBeDisabled();
    fireEvent.change(within(form).getByLabelText('规范名称'), { target: { value: ' 示例营 ' } });
    fireEvent.change(within(form).getByLabelText(/^可用叫法/), { target: { value: '示例陪跑营\n  \n示例营学员' } });
    fireEvent.change(within(form).getByLabelText(/不要用的叫法/), { target: { value: '示例课' } });
    vi.mocked(api.listFactCards).mockResolvedValue(views(card(), null));
    fireEvent.click(within(form).getByRole('button', { name: '保存' }));
    await waitFor(() => expect(api.saveFactCard).toHaveBeenCalledTimes(1));
    const [code, command] = vi.mocked(api.saveFactCard).mock.calls[0]!;
    expect(code).toBeNull();
    expect(command).toMatchObject({ expectedRevision: 0, name: '示例营', allowedNames: ['示例陪跑营', '示例营学员'], bannedNames: ['示例课'] });
    const saved = await screen.findByRole('form', { name: '产品事实卡 CP-0001' });
    expect(within(saved).getByText(/这一版还没有确认/)).toBeInTheDocument();
    expect(api.confirmFactCard).not.toHaveBeenCalled();
    fireEvent.click(within(saved).getByRole('button', { name: '确认这一版' }));
    await waitFor(() => expect(api.confirmFactCard).toHaveBeenCalledWith('CP-0001', expect.objectContaining({ expectedRevision: 1 })));
  });

  it('writes the confirmation time the way the rest of the site does', async () => {
    const confirmed = card({ confirmedAt: '2026-10-28 15:23:39.566182+08' });
    vi.mocked(api.listFactCards).mockResolvedValue(views(confirmed, confirmed));
    show();
    open('产品事实卡');
    fireEvent.click(await screen.findByRole('button', { name: /示例营/ }));
    expect(await screen.findByText('这一版已确认（2026-10-28 15:23），检查按它执行。')).toBeInTheDocument();
  });

  it('saves an edit as the next revision of the same card', async () => {
    vi.mocked(api.listFactCards).mockResolvedValue(views(card({ revision: 3, confirmedAt: 't' }), card({ revision: 3, confirmedAt: 't' })));
    vi.mocked(api.saveFactCard).mockResolvedValue({ card: card({ revision: 4 }), replayed: false });
    show();
    open('产品事实卡');
    fireEvent.click(await screen.findByRole('button', { name: /示例营/ }));
    const form = await screen.findByRole('form', { name: '产品事实卡 CP-0001' });
    expect(within(form).queryByRole('button', { name: '确认这一版' })).not.toBeInTheDocument();
    fireEvent.change(within(form).getByLabelText('规范名称'), { target: { value: '示例营（新）' } });
    fireEvent.click(within(form).getByRole('button', { name: '保存为新的一版' }));
    await waitFor(() => expect(api.saveFactCard).toHaveBeenCalledWith('CP-0001', expect.objectContaining({ expectedRevision: 3, name: '示例营（新）' })));
  });
});

describe('衔接句', () => {
  it('blocks a sentence with a cure promise and explains which words, without a way to force it in', async () => {
    vi.mocked(api.checkSentence).mockResolvedValue({
      rulesetVersion: 'v', blocked: true, notRun: [],
      findings: [{ rule: 'R-L1', level: 'must', message: '“保证能好”是疗效承诺，衔接句里不能写，不能加入', location: { kind: 'text', start: 4, end: 8 }, fixes: [] }],
    });
    show();
    open('衔接句');
    fireEvent.click(await screen.findByRole('button', { name: '＋ 新增一句' }));
    const form = await screen.findByRole('form', { name: '新增衔接句' });
    fireEvent.change(within(form).getByLabelText(/这句话/), { target: { value: '我家孩子保证能好' } });
    expect(await within(form).findByText(/疗效承诺，衔接句里不能写/)).toBeInTheDocument();
    expect(within(form).getByRole('alert')).toHaveTextContent('含有疗效承诺，不能加入句库');
    expect(within(form).getByRole('button', { name: '加入句库（待审）' })).toBeDisabled();
  });

  it('adds a sentence only after its check came back, and shows reminders without blocking', async () => {
    vi.mocked(api.checkSentence).mockResolvedValue({
      rulesetVersion: 'v', blocked: false, notRun: [{ rule: 'R-L4', reason: '产品事实卡还没有确认，产品叫法没有检查' }],
      findings: [{ rule: 'R-L7', level: 'warn', message: '这句有 80 个字，建议不超过 70 个字', location: { kind: 'text', start: 0, end: 80 }, fixes: [] }],
    });
    vi.mocked(api.createSentence).mockResolvedValue({ sentence: sentence(), replayed: false });
    show();
    open('衔接句');
    fireEvent.click(await screen.findByRole('button', { name: '＋ 新增一句' }));
    const form = await screen.findByRole('form', { name: '新增衔接句' });
    fireEvent.change(within(form).getByLabelText('适合哪种身份'), { target: { value: 'coach' } });
    fireEvent.change(within(form).getByLabelText(/这句话/), { target: { value: ' 我是陪跑老师 ' } });
    expect(within(form).getByRole('button', { name: '加入句库（待审）' })).toBeDisabled();
    expect(await within(form).findByText(/这句有 80 个字/)).toBeInTheDocument();
    expect(within(form).getByText(/未检查：产品事实卡还没有确认，产品叫法没有检查。/)).toBeInTheDocument();
    await waitFor(() => expect(within(form).getByRole('button', { name: '加入句库（待审）' })).toBeEnabled());
    fireEvent.click(within(form).getByRole('button', { name: '加入句库（待审）' }));
    await waitFor(() => expect(api.createSentence).toHaveBeenCalledWith(expect.objectContaining({ group: 'coach', text: '我是陪跑老师', origin: 'manual' })));
  });

  it('shows a refused add with the server findings even if the live check had passed', async () => {
    vi.mocked(api.createSentence).mockRejectedValue(new ApiError({ kind: 'http', status: 409, code: 'creation_assets_blocked' },
      { code: 'creation_assets_blocked', findings: [{ rule: 'R-L1', level: 'must', message: '“治愈”是疗效承诺', location: { kind: 'text', start: 0, end: 2 }, fixes: [] }] }));
    show();
    open('衔接句');
    fireEvent.click(await screen.findByRole('button', { name: '＋ 新增一句' }));
    const form = await screen.findByRole('form', { name: '新增衔接句' });
    fireEvent.change(within(form).getByLabelText(/这句话/), { target: { value: '孩子治愈了' } });
    await waitFor(() => expect(within(form).getByRole('button', { name: '加入句库（待审）' })).toBeEnabled());
    fireEvent.click(within(form).getByRole('button', { name: '加入句库（待审）' }));
    expect(await within(form).findByText('“治愈”是疗效承诺')).toBeInTheDocument();
  });

  it('lists each sentence with its code, voice, status and how many drafts use it, and filters on the server', async () => {
    vi.mocked(api.listSentences).mockResolvedValue([sentence({ status: 'ok' }), sentence({ code: 'XJ-0002', group: 'coach', text: '陪跑老师的一句', usedCount: 0 })]);
    show();
    open('衔接句');
    expect(await screen.findByText('稿件选用 3 篇')).toBeInTheDocument();
    expect(screen.getByText('陪跑老师的一句')).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('按身份筛选'), { target: { value: 'coach' } });
    fireEvent.change(screen.getByLabelText('按状态筛选'), { target: { value: 'ok' } });
    await waitFor(() => expect(api.listSentences).toHaveBeenLastCalledWith({ group: 'coach', status: 'ok' }, expect.anything()));
  });

  it('reviews with the actions that fit the status, and sending back needs a reason', async () => {
    vi.mocked(api.listSentences).mockResolvedValue([sentence()]);
    vi.mocked(api.reviewSentence).mockResolvedValue({ sentence: sentence({ status: 'ok' }), replayed: false });
    show();
    open('衔接句');
    fireEvent.click(await screen.findByRole('button', { name: /我也是学员家长/ }));
    const review = await screen.findByRole('region', { name: '审核' });
    expect(within(review).getByText(/现在是“待审”/)).toBeInTheDocument();
    expect(within(review).queryByRole('button', { name: /重新启用/ })).not.toBeInTheDocument();
    expect(within(review).getByRole('button', { name: '退回待改' })).toBeDisabled();
    fireEvent.change(within(review).getByLabelText(/说明/), { target: { value: '太像招揽' } });
    expect(within(review).getByRole('button', { name: '退回待改' })).toBeEnabled();
    fireEvent.click(within(review).getByRole('button', { name: '审核通过' }));
    await waitFor(() => expect(api.reviewSentence).toHaveBeenCalledWith('XJ-0001', expect.objectContaining({ expectedRevision: 1, action: 'approve', note: '太像招揽' })));
  });

  it('offers only re-enabling for a stopped sentence and saves a rewrite as a new revision', async () => {
    vi.mocked(api.listSentences).mockResolvedValue([sentence({ status: 'off' })]);
    vi.mocked(api.editSentence).mockResolvedValue({ sentence: sentence({ revision: 2 }), replayed: false });
    show();
    open('衔接句');
    fireEvent.click(await screen.findByRole('button', { name: /我也是学员家长/ }));
    const review = await screen.findByRole('region', { name: '审核' });
    expect(within(review).getByRole('button', { name: '重新启用（回到待审）' })).toBeInTheDocument();
    expect(within(review).queryByRole('button', { name: '审核通过' })).not.toBeInTheDocument();
    const editor = screen.getByRole('form', { name: '改写 XJ-0001' });
    expect(within(editor).getByRole('button', { name: '保存为新的一版（回到待审）' })).toBeDisabled();
    fireEvent.change(within(editor).getByLabelText(/这句话/), { target: { value: '我也是学员家长，慢慢来' } });
    await waitFor(() => expect(within(editor).getByRole('button', { name: '保存为新的一版（回到待审）' })).toBeEnabled());
    fireEvent.click(within(editor).getByRole('button', { name: '保存为新的一版（回到待审）' }));
    await waitFor(() => expect(api.editSentence).toHaveBeenCalledWith('XJ-0001', expect.objectContaining({ expectedRevision: 1, text: '我也是学员家长，慢慢来' })));
  });
});
