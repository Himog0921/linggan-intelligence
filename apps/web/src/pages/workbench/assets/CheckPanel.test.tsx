import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '../../../shared/api';
import { newDocument, type Revision, type WorkDocument } from '../workbenchModel';
import * as api from './assetsApi';
import type { AccountView, CheckReport, Finding, Identity, Sentence } from './assetsModel';
import { CheckPanel } from './CheckPanel';

vi.mock('./assetsApi', async importOriginal => {
  const original = await importOriginal<typeof import('./assetsApi')>();
  return {
    ...original, listAccounts: vi.fn(), checkDraft: vi.fn(), candidates: vi.fn(), copyStatus: vi.fn(),
    prepareCopy: vi.fn(), confirmCopy: vi.fn(), putIdentity: vi.fn(), createSentence: vi.fn(),
  };
});

const WORK = '00000000-0000-4000-8000-000000000010';
const DOMAIN = '00000000-0000-4000-8000-000000000011';
const ACCOUNT = '00000000-0000-4000-8000-000000000012';
const P1 = '00000000-0000-4000-8000-000000000013';
const REV = '00000000-0000-4000-8000-000000000014';
const RECEIPT = '00000000-0000-4000-8000-000000000015';

const identity = (patch: Partial<Identity> = {}): Identity => ({
  accountRef: ACCOUNT, revision: 1, orgRelation: 'parent', bioDisclosesRelation: true, bioCheckedAt: 't', bioCheckedBy: 'local_machine',
  hasRealPerson: true, backingPersonRole: '妈妈', canProvideExperience: null, savedAt: 't', savedBy: 'local_machine', ...patch,
});
const account = (patch: Partial<AccountView> = {}): AccountView => ({
  accountRef: ACCOUNT, accountName: '示例账号', platform: 'xhs', status: 'active', operatorName: null, identity: identity(), ...patch,
});
const sentence = (code: string, text: string): Sentence => ({
  code, revision: 1, group: 'parent', text, status: 'ok', statusNote: null, origin: 'manual', entryFindings: [], usedCount: 0, savedAt: 't', savedBy: 'local_machine',
});
const finding = (patch: Partial<Finding> & Pick<Finding, 'rule' | 'level' | 'message'>): Finding => ({ location: { kind: 'slot' }, fixes: [], ...patch });
const report = (findings: Finding[], patch: Partial<CheckReport> = {}): CheckReport => ({
  rulesetVersion: 'creation-checks.v1', bridgeState: 'ready', bridgeGroup: 'parent', findings, notRun: [], factCards: [], ...patch,
});
const doc = (patch: Partial<WorkDocument> = {}): WorkDocument => ({
  ...newDocument('合成标题'), accountRef: ACCOUNT, paragraphs: [{ id: P1, text: '甲😀孩子变好了，真好' }], ...patch,
});
const revisionOf = (document: WorkDocument): Revision => ({ id: REV, number: 3, savedAt: 't', document });

type Props = Partial<Parameters<typeof CheckPanel>[0]>;
function show(props: Props = {}) {
  const document = props.document ?? doc();
  const edit = vi.fn();
  const saveCurrent = vi.fn(async () => revisionOf(document));
  const onTagsChange = vi.fn();
  const onGoCover = vi.fn();
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const element = (next: Props = {}) => <QueryClientProvider client={client}>
    <CheckPanel workId={WORK} domainRef={DOMAIN} document={document} revision={revisionOf(document)} saveState="saved" composing={false}
      locked={false} tagsDraft="" onTagsChange={onTagsChange} edit={edit} saveCurrent={saveCurrent} onGoCover={onGoCover} {...props} {...next} />
  </QueryClientProvider>;
  const view = render(element());
  return { ...view, edit, saveCurrent, onTagsChange, onGoCover, rerenderWith: (next: Props) => view.rerender(element(next)) };
}
const clipboard = { writeText: vi.fn() };

beforeEach(() => {
  vi.resetAllMocks();
  clipboard.writeText.mockResolvedValue(undefined);
  Object.defineProperty(navigator, 'clipboard', { value: clipboard, configurable: true });
  vi.mocked(api.listAccounts).mockResolvedValue([account()]);
  vi.mocked(api.candidates).mockResolvedValue({ state: 'ready', group: 'parent', items: [sentence('XJ-0001', '我也是学员家长'), sentence('XJ-0002', '一起慢慢来')] });
  vi.mocked(api.copyStatus).mockResolvedValue({ copied: null });
  vi.mocked(api.checkDraft).mockResolvedValue(report([]));
});
afterEach(() => vi.useRealTimers());

const ready = async () => { await screen.findByText(/必改 \d+ · 提醒 \d+ · 提示 \d+$/); };

describe('check results', () => {
  it('groups findings by level with a word for each level, and counts them', async () => {
    vi.mocked(api.checkDraft).mockResolvedValue(report([
      finding({ rule: 'R-M4', level: 'must', message: '还没选衔接句' }),
      finding({ rule: 'R-W1', level: 'warn', message: '“变好了”是在讲效果' }),
      finding({ rule: 'R-W2', level: 'warn', message: '对所有孩子下结论' }),
      finding({ rule: 'R-I1', level: 'hint', message: '封面是 AI 作图' }),
    ]));
    show();
    await screen.findByText('必改 1 · 提醒 2 · 提示 1');
    for (const [heading, text] of [['必须先改', '还没选衔接句'], ['提醒', '“变好了”是在讲效果'], ['发布时记得', '封面是 AI 作图']] as const) {
      const group = screen.getByRole('region', { name: heading });
      expect(within(group).getByText(text)).toBeInTheDocument();
    }
    expect(within(screen.getByRole('region', { name: '必须先改' })).getByText('必改')).toBeInTheDocument();
  });

  it('says so when nothing was found, and lists the rules that could not run', async () => {
    vi.mocked(api.checkDraft).mockResolvedValue(report([], { notRun: [{ rule: 'R-W3', reason: '产品事实卡还没有确认，产品叫法没有检查' }] }));
    show();
    await screen.findByText('没有发现问题。');
    expect(screen.getByText(/未检查（R-W3）：产品事实卡还没有确认/)).toBeInTheDocument();
  });

  it('shows a failed check as a failure with a retry, never as "nothing found", and blocks copying', async () => {
    vi.mocked(api.checkDraft).mockRejectedValueOnce(new ApiError({ kind: 'network' }));
    show();
    await screen.findByRole('alert');
    expect(screen.getByRole('alert')).toHaveTextContent('检查读取失败');
    expect(screen.queryByText('没有发现问题。')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '复制到小红书' })).toBeDisabled();
    vi.mocked(api.checkDraft).mockResolvedValue(report([]));
    fireEvent.click(within(screen.getByRole('alert')).getByRole('button', { name: '重试' }));
    await screen.findByText('没有发现问题。');
    expect(screen.getByRole('button', { name: '复制到小红书' })).toBeEnabled();
  });

  it('sends only the check document and holds new checks while a Chinese input is open', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    const first = doc();
    const view = show({ document: first });
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(vi.mocked(api.checkDraft)).toHaveBeenCalledTimes(1);
    const typed = { ...first, paragraphs: [{ id: P1, text: '甲😀孩子变好了，真好，nihao' }] };
    view.rerenderWith({ document: typed, composing: true });
    await act(async () => { await vi.advanceTimersByTimeAsync(2000); });
    expect(vi.mocked(api.checkDraft)).toHaveBeenCalledTimes(1);
    view.rerenderWith({ document: typed, composing: false });
    await act(async () => { await vi.advanceTimersByTimeAsync(600); });
    expect(vi.mocked(api.checkDraft)).toHaveBeenCalledTimes(2);
    expect(vi.mocked(api.checkDraft).mock.calls[1]?.[0].paragraphs[0]?.text).toContain('nihao');
  });
});

describe('fixes', () => {
  const at = (text: string, needle: string) => ({ start: text.indexOf(needle), end: text.indexOf(needle) + needle.length });

  it('deletes and replaces located text in UTF-16 units through the ordinary edit', async () => {
    const text = '甲😀孩子变好了，真好';
    const span = at(text, '变好了');
    vi.mocked(api.checkDraft).mockResolvedValue(report([
      finding({ rule: 'R-W10', level: 'warn', message: '疗效承诺', location: { kind: 'paragraph', id: P1, ...span }, fixes: [{ label: '删掉这几个字', action: 'deleteMatch' }] }),
      finding({ rule: 'R-W6', level: 'warn', message: '写法不统一', location: { kind: 'paragraph', id: P1, ...at(text, '孩子') }, fixes: [{ label: '一键统一', action: 'replaceText', replacement: 'A娃' }] }),
      finding({ rule: 'R-W11', level: 'warn', message: '标题疗效承诺', location: { kind: 'title', start: 1, end: 3 }, fixes: [{ label: '改标题', action: 'replaceText', replacement: '甲' }] }),
    ]));
    const { edit } = show({ document: doc({ title: '三天治愈' }) });
    await ready();
    fireEvent.click(screen.getByRole('button', { name: '删掉这几个字' }));
    expect(edit).toHaveBeenLastCalledWith(expect.objectContaining({ paragraphs: [{ id: P1, text: '甲😀孩子，真好' }] }));
    fireEvent.click(screen.getByRole('button', { name: '一键统一' }));
    expect(edit).toHaveBeenLastCalledWith(expect.objectContaining({ paragraphs: [{ id: P1, text: '甲😀A娃变好了，真好' }] }));
    fireEvent.click(screen.getByRole('button', { name: '改标题' }));
    expect(edit).toHaveBeenLastCalledWith(expect.objectContaining({ title: '三甲愈' }));
  });

  it('never rewrites text while the shown results are older than the text on screen', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    const text = '甲😀孩子变好了，真好';
    vi.mocked(api.checkDraft).mockResolvedValue(report([
      finding({ rule: 'R-W10', level: 'warn', message: '疗效承诺', location: { kind: 'paragraph', id: P1, ...at(text, '变好了') }, fixes: [{ label: '删掉这几个字', action: 'deleteMatch' }] }),
    ]));
    const first = doc();
    const view = show({ document: first });
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(screen.getByRole('button', { name: '删掉这几个字' })).toBeEnabled();
    view.rerenderWith({ document: { ...first, paragraphs: [{ id: P1, text: `新增一段字，${first.paragraphs[0]?.text}` }] } });
    expect(screen.getByRole('button', { name: '删掉这几个字' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '复制到小红书' })).toBeDisabled();
    expect(screen.getByText(/（更新中）/)).toBeInTheDocument();
  });

  it('skips the bridge, removes a repeated topic and jumps to the cover headline', async () => {
    vi.mocked(api.checkDraft).mockResolvedValue(report([
      finding({ rule: 'R-M4', level: 'must', message: '还没选衔接句', fixes: [{ label: '这篇不植入', action: 'skipBridge' }] }),
      finding({ rule: 'R-W7', level: 'warn', message: '话题重复', location: { kind: 'tag', index: 1 }, fixes: [{ label: '删掉重复的', action: 'removeTag' }] }),
      finding({ rule: 'R-W8', level: 'warn', message: '封面大字太长', location: { kind: 'cover', start: 0, end: 12 }, fixes: [] }),
    ]));
    const { edit, onTagsChange, onGoCover } = show({ tagsDraft: 'ADHD adhd 放手' });
    await ready();
    fireEvent.click(screen.getByRole('button', { name: '这篇不植入' }));
    expect(edit).toHaveBeenLastCalledWith(expect.objectContaining({ bridge: { skip: true, sentence: null, afterParagraph: null } }));
    fireEvent.click(screen.getByRole('button', { name: '删掉重复的' }));
    expect(onTagsChange).toHaveBeenCalledWith('ADHD 放手');
    const coverItem = screen.getByText('封面大字太长').closest('li') as HTMLElement;
    fireEvent.click(within(coverItem).getByRole('button', { name: '找到位置' }));
    expect(onGoCover).toHaveBeenCalledTimes(1);
  });

  it('picks the first usable sentence for an empty slot and the next one when swapping', async () => {
    vi.mocked(api.checkDraft).mockResolvedValue(report([
      finding({ rule: 'R-M4', level: 'must', message: '还没选衔接句', fixes: [{ label: '自动选一句', action: 'autoPickBridge' }] }),
    ]));
    const { edit } = show();
    await ready();
    fireEvent.click(await screen.findByRole('button', { name: '自动选一句' }));
    await waitFor(() => expect(edit).toHaveBeenCalledTimes(1));
    expect(edit.mock.calls[0]?.[0].bridge).toEqual({ skip: false, afterParagraph: null,
      sentence: { sentenceCode: 'XJ-0001', sentenceRevision: 1, group: 'parent', text: '我也是学员家长', edited: false } });
  });

  it('keeps an empty library honest instead of inventing a sentence', async () => {
    vi.mocked(api.candidates).mockResolvedValue({ state: 'ready', group: 'parent', items: [] });
    vi.mocked(api.checkDraft).mockResolvedValue(report([
      finding({ rule: 'R-M4', level: 'must', message: '还没选衔接句', fixes: [{ label: '自动选一句', action: 'autoPickBridge' }] }),
    ]));
    const { edit } = show();
    await ready();
    fireEvent.click(await screen.findByRole('button', { name: '自动选一句' }));
    await screen.findAllByText(/还没有审核通过的衔接句/);
    expect(edit).not.toHaveBeenCalled();
  });

  it('copies the suggested bio and asks before recording that the bio was rewritten', async () => {
    vi.mocked(api.checkDraft).mockResolvedValue(report([
      finding({ rule: 'R-M2', level: 'must', message: '简介还没写明', fixes: [
        { label: '复制建议简介', action: 'copyBioSuggestion', text: '我是示例营的学员家长' }, { label: '我已改好简介', action: 'confirmBioWritten' }] }),
    ], { bridgeState: 'bio_missing' }));
    vi.mocked(api.putIdentity).mockResolvedValue({ identity: identity({ revision: 2 }), replayed: false });
    show();
    await ready();
    fireEvent.click(screen.getByRole('button', { name: '复制建议简介' }));
    await screen.findByText(/已复制建议简介/);
    expect(clipboard.writeText).toHaveBeenCalledWith('我是示例营的学员家长');
    fireEvent.click(screen.getByRole('button', { name: '我已改好简介' }));
    expect(vi.mocked(api.putIdentity)).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole('button', { name: '确认已写明' }));
    await waitFor(() => expect(vi.mocked(api.putIdentity)).toHaveBeenCalledTimes(1));
    const [accountRef, command] = vi.mocked(api.putIdentity).mock.calls[0]!;
    expect(accountRef).toBe(ACCOUNT);
    expect(command).toMatchObject({ expectedRevision: 1, orgRelation: 'parent', bioDisclosesRelation: true, hasRealPerson: true });
  });
});

describe('copy', () => {
  it('stays closed until 必改 is clear and says why', async () => {
    vi.mocked(api.checkDraft).mockResolvedValue(report([finding({ rule: 'R-M1', level: 'must', message: '账号身份还没确认' })]));
    show();
    await screen.findByText('必改 1 · 提醒 0 · 提示 0');
    expect(screen.getByRole('button', { name: '复制到小红书' })).toBeDisabled();
    expect(screen.getByText('必改清零后才能复制')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '复制标题' })).toBeDisabled();
  });

  it('saves first, checks the saved revision on the server, writes the clipboard, then records the copy', async () => {
    vi.mocked(api.checkDraft).mockResolvedValue(report([finding({ rule: 'R-W1', level: 'warn', message: '讲效果' })]));
    vi.mocked(api.prepareCopy).mockResolvedValue({ receiptRef: RECEIPT, workId: WORK, revisionId: REV, rulesetVersion: 'creation-checks.v1',
      title: '合成标题', body: '正文', tags: [], text: '甲\nㅤ\n乙', warnCount: 1, hintCount: 0, replayed: false });
    vi.mocked(api.confirmCopy).mockResolvedValue({ receiptRef: RECEIPT, revisionId: REV, copiedAt: 't' });
    const { saveCurrent } = show({ saveState: 'dirty' });
    await screen.findByText('必改 0 · 提醒 1 · 提示 0');
    fireEvent.click(screen.getByRole('button', { name: '复制到小红书' }));
    await screen.findByText(/已复制到剪贴板（还有 1 条提醒没处理）/);
    expect(saveCurrent).toHaveBeenCalledTimes(1);
    expect(vi.mocked(api.prepareCopy)).toHaveBeenCalledWith(WORK, expect.objectContaining({ expectedRevision: REV }));
    expect(clipboard.writeText).toHaveBeenCalledWith('甲\nㅤ\n乙');
    expect(vi.mocked(api.confirmCopy)).toHaveBeenCalledWith(WORK, RECEIPT, expect.objectContaining({ requestId: expect.any(String) }));
    expect(screen.getByText(/这不等于已发布/)).toBeInTheDocument();
  });

  it('does not copy or claim a copy when the save did not complete', async () => {
    const { saveCurrent } = show();
    await ready();
    saveCurrent.mockRejectedValueOnce(new Error('conflict'));
    fireEvent.click(screen.getByRole('button', { name: '复制到小红书' }));
    await screen.findByText(/保存没有完成，所以没有复制/);
    expect(vi.mocked(api.prepareCopy)).not.toHaveBeenCalled();
    expect(clipboard.writeText).not.toHaveBeenCalled();
  });

  it('reports a refusal from the server as a refusal and re-reads the results', async () => {
    vi.mocked(api.prepareCopy).mockRejectedValue(new ApiError({ kind: 'http', status: 409, code: 'creation_assets_blocked' },
      { code: 'creation_assets_blocked', findings: [{ rule: 'R-M2', level: 'must', message: '简介还没写明', location: { kind: 'slot' }, fixes: [] }] }));
    show();
    await ready();
    const before = vi.mocked(api.checkDraft).mock.calls.length;
    fireEvent.click(screen.getByRole('button', { name: '复制到小红书' }));
    await screen.findByText('还有必须先改的问题，没有复制。');
    expect(clipboard.writeText).not.toHaveBeenCalled();
    expect(vi.mocked(api.confirmCopy)).not.toHaveBeenCalled();
    await waitFor(() => expect(vi.mocked(api.checkDraft).mock.calls.length).toBeGreaterThan(before));
  });

  it('falls back to a manual copy window and only records the copy after the writer confirms', async () => {
    clipboard.writeText.mockRejectedValue(new Error('denied'));
    vi.mocked(api.prepareCopy).mockResolvedValue({ receiptRef: RECEIPT, workId: WORK, revisionId: REV, rulesetVersion: 'v', title: 't', body: 'b',
      tags: [], text: '要手动复制的文字', warnCount: 0, hintCount: 0, replayed: false });
    vi.mocked(api.confirmCopy).mockResolvedValue({ receiptRef: RECEIPT, revisionId: REV, copiedAt: 't' });
    show();
    await ready();
    fireEvent.click(screen.getByRole('button', { name: '复制到小红书' }));
    expect(await screen.findByLabelText('待复制的文字')).toHaveValue('要手动复制的文字');
    expect(vi.mocked(api.confirmCopy)).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '我已复制' }));
    await waitFor(() => expect(vi.mocked(api.confirmCopy)).toHaveBeenCalledTimes(1));
    await screen.findByText(/已记录为已复制/);
  });

  it('tells "已复制" from "草稿中" by the revision, and drops back after another edit', async () => {
    vi.mocked(api.copyStatus).mockResolvedValue({ copied: { receiptRef: RECEIPT, revisionId: REV, copiedAt: 't' } });
    const view = show();
    await screen.findByText('已复制（还没有发布）');
    view.rerenderWith({ saveState: 'dirty' });
    expect(screen.getByText('草稿中')).toBeInTheDocument();
  });

  it('copies the title alone without recording a copy of the note', async () => {
    show({ document: doc({ title: '  合成标题  ' }) });
    await ready();
    fireEvent.click(screen.getByRole('button', { name: '复制标题' }));
    await screen.findByText('标题已复制。');
    expect(clipboard.writeText).toHaveBeenCalledWith('合成标题');
    expect(vi.mocked(api.prepareCopy)).not.toHaveBeenCalled();
  });
});

describe('account and bridge slot', () => {
  it('changes the account through the ordinary edit and offers the identity editor', async () => {
    vi.mocked(api.listAccounts).mockResolvedValue([account(), account({ accountRef: '00000000-0000-4000-8000-000000000099', accountName: '另一个号', identity: null })]);
    const { edit } = show();
    await ready();
    fireEvent.change(screen.getByLabelText('所属账号'), { target: { value: '00000000-0000-4000-8000-000000000099' } });
    expect(edit).toHaveBeenLastCalledWith(expect.objectContaining({ accountRef: '00000000-0000-4000-8000-000000000099' }));
    fireEvent.click(screen.getByRole('button', { name: '编辑账号身份' }));
    expect(await screen.findByRole('heading', { name: /示例账号 · 账号身份/ })).toBeInTheDocument();
  });

  it('shows the chosen sentence, cycles the library and marks a rewrite as local', async () => {
    const chosen = { skip: false, afterParagraph: null, sentence: { sentenceCode: 'XJ-0001', sentenceRevision: 1, group: 'parent' as const, text: '我也是学员家长', edited: false } };
    const { edit } = show({ document: doc({ bridge: chosen }) });
    await ready();
    const slot = await screen.findByRole('region', { name: '衔接位' });
    await within(slot).findByRole('button', { name: '换一句 1/2' });
    fireEvent.click(within(slot).getByRole('button', { name: '换一句 1/2' }));
    expect(edit.mock.calls.at(-1)?.[0].bridge.sentence).toMatchObject({ sentenceCode: 'XJ-0002', text: '一起慢慢来', edited: false });
    fireEvent.change(within(slot).getByLabelText('本篇的衔接句'), { target: { value: '我改写了这一句' } });
    expect(edit.mock.calls.at(-1)?.[0].bridge.sentence).toMatchObject({ sentenceCode: 'XJ-0001', text: '我改写了这一句', edited: true });
    fireEvent.change(within(slot).getByLabelText('衔接句的位置'), { target: { value: P1 } });
    expect(edit.mock.calls.at(-1)?.[0].bridge.afterParagraph).toBe(P1);
    fireEvent.click(within(slot).getByRole('button', { name: '这篇不植入' }));
    expect(edit.mock.calls.at(-1)?.[0].bridge.skip).toBe(true);
  });

  it('saves a rewritten sentence back to the library as waiting for review', async () => {
    vi.mocked(api.createSentence).mockResolvedValue({ sentence: sentence('XJ-0007', '改写后'), replayed: false });
    const edited = { skip: false, afterParagraph: null, sentence: { sentenceCode: 'XJ-0001', sentenceRevision: 1, group: 'parent' as const, text: '改写后', edited: true } };
    show({ document: doc({ bridge: edited }) });
    await ready();
    fireEvent.click(await screen.findByRole('button', { name: '存回衔接句库（待审）' }));
    await screen.findByText(/已存回衔接句库（XJ-0007）/);
    expect(vi.mocked(api.createSentence)).toHaveBeenCalledWith(expect.objectContaining({ group: 'parent', text: '改写后', origin: 'saved_back' }));
  });

  it('offers to restore a skipped slot', async () => {
    vi.mocked(api.checkDraft).mockResolvedValue(report([], { bridgeState: 'skipped' }));
    const { edit } = show({ document: doc({ bridge: { skip: true, sentence: null, afterParagraph: null } }) });
    await ready();
    fireEvent.click(await screen.findByRole('button', { name: '恢复衔接' }));
    expect(edit.mock.calls.at(-1)?.[0].bridge.skip).toBe(false);
  });
});
