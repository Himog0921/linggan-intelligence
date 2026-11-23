import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useCallback, useRef, useState } from 'react';
import { Link, useBeforeUnload, useBlocker, useNavigate, useSearchParams } from 'react-router';
import { ActionButton } from '../../shared/ActionButton';
import { ApiError } from '../../shared/api';
import { OpsFrame } from '../ops/OpsFrame';
import { OpsScopePicker } from '../ops/OpsScopePicker';
import { CreationPublicationButton } from './CreationPublicationButton';
import { creationPublicationHref } from '../ops/api';
import { newDocument, type WorkDocument } from './workbenchModel';
import * as api from './memberWorkbenchApi';
import type { Workspace, Draft } from './memberWorkbenchApi';
import styles from './WorkbenchPage.module.css';

type Submission = { requestId: string; document: WorkDocument };

function errorMessage(error: unknown) {
  if (error instanceof ApiError && error.failure.kind === 'http') {
    if (error.failure.status === 401) return '会话已失效，请重新登录。';
    if (error.failure.status === 403) return '当前成员无权完成这项操作。';
    if (error.failure.status === 409) return '稿件版本已变化，请重新打开稿件核对。';
  }
  return '操作未完成。请重试。';
}

export function MemberWorkbench({ memberRef }: { memberRef: string }) {
  const [params, setParams] = useSearchParams();
  const navigate = useNavigate();
  const [filter, setFilter] = useState<'mine' | 'team'>('mine');
  const [title, setTitle] = useState('');
  const [creating, setCreating] = useState(false);
  const [unconfirmed, setUnconfirmed] = useState(false);
  const [error, setError] = useState('');
  const [assetAccount, setAssetAccount] = useState<string | null>(null);
  const pending = useRef<(Submission & { domainRef: string }) | null>(null);
  const hasPendingCreation = useCallback(() => pending.current !== null, []);
  const client = useQueryClient();
  const domain = params.get('domain') ?? '';
  const work = params.get('work');
  const section = work || params.get('section') === 'works' ? 'works' : 'today';
  const domains = useQuery({
    queryKey: ['member-creation', memberRef, 'domains'],
    queryFn: ({ signal }) => api.readDomains(signal),
    gcTime: 0,
  });
  const activeDomain = domains.data?.some(item => item.domainRef === domain) === true;
  const workspace = useQuery({
    queryKey: ['member-creation', memberRef, 'workspace', domain, filter],
    queryFn: ({ signal }) => api.readWorkspace(domain, filter === 'mine', signal),
    enabled: activeDomain, gcTime: 0,
  });
  const draftKey = ['member-creation', memberRef, 'draft', work];
  const draft = useQuery({
    queryKey: draftKey,
    queryFn: ({ signal }) => {
      if (!work) throw new Error('未选择稿件');
      return api.readDraft(work, signal);
    },
    enabled: activeDomain && !!work, gcTime: 0,
  });

  function open(workId: string) {
    setParams({ domain, work: workId, view: 'body' });
  }

  async function create() {
    if (creating) return;
    const command = pending.current ?? { requestId: crypto.randomUUID(), domainRef: domain, document: newDocument(title.trim()) };
    pending.current = command;
    setCreating(true);
    setError('');
    try {
      const receipt = await api.create(command);
      pending.current = null;
      setUnconfirmed(false);
      setTitle('');
      void client.invalidateQueries({ queryKey: ['member-creation', memberRef, 'workspace', domain] });
      open(receipt.workId);
    } catch (failure) {
      const answered = failure instanceof ApiError && failure.failure.kind === 'http' && failure.failure.status < 500;
      if (answered) pending.current = null;
      setUnconfirmed(!answered);
      setError(answered ? errorMessage(failure) : '新建稿件的结果尚未确认。请沿用原请求重试。');
    } finally {
      setCreating(false);
    }
  }

  const firstAccount = workspace.data?.accounts[0];
  return <OpsFrame section="creation" domain={domain} restricted creationPageKey={section} onNavigate={href => { void navigate(href); }}
    scope={<OpsScopePicker creation options={domains.data ?? []} value={domain} loading={domains.isPending} failed={domains.isError}
      disabled={creating || unconfirmed} onChange={next => { setAssetAccount(null); setParams({ domain: next, section }); }} />}
    meta={firstAccount ? <ActionButton variant="outline" onClick={() => setAssetAccount(assetAccount ? null : firstAccount.ref)}>账号资产</ActionButton> : null}>
    <main className={styles.workspace} aria-label="运营创作">
      {domains.isError ? <p role="alert">创作领域读取失败。</p> : domains.isPending ? <p role="status">正在读取创作领域…</p>
        : !activeDomain ? <section className={styles.library}><h1>选择创作领域</h1><p>从页面上方选择领域，查看稿件或开始写作。</p></section>
        : <section className={styles.library}>
          {assetAccount && workspace.data?.accounts.some(account => account.ref === assetAccount) ?
            <AccountAssets key={assetAccount} memberRef={memberRef} accountRef={assetAccount} accounts={workspace.data.accounts} onChange={setAssetAccount} /> : null}
          {error ? <p role="alert" className={styles.error}>{error}</p> : null}
          {workspace.isError ? <p role="alert">创作列表暂时无法读取。<ActionButton onClick={() => void workspace.refetch()}>重新读取</ActionButton></p> : null}
          {work ? <>
            <Link to={`/ops/creation?${new URLSearchParams({ domain, section: 'works' })}`}>返回我的创作</Link>
            {draft.isPending ? <p role="status">正在读取稿件…</p> : draft.isError ? <p role="alert">{errorMessage(draft.error)}</p> :
              draft.data.domainRef !== domain ? <p role="alert">稿件不属于当前领域。</p> :
              <MemberDraft key={work} draft={draft.data} accounts={workspace.data?.accounts ?? []}
                refresh={() => { void client.invalidateQueries({ queryKey: draftKey }); void workspace.refetch(); }} />}
          </> : section === 'today' ? <>
            <h1>今日选题</h1>
            {workspace.isPending ? <p role="status">正在读取选题…</p> :
              workspace.data?.ideas.length === 0 ? <p>当前领域还没有已保存的选题。</p> : null}
            {workspace.data?.ideas.map(idea => <article key={idea.ideaRef}>
              <h2><button type="button" onClick={() => open(idea.anchorWorkId)}>{idea.name}</button></h2>
              <p>{idea.intent}</p>
            </article>)}
            <Link to={`/ops/creation?${new URLSearchParams({ domain, section: 'works' })}`}>去我的创作</Link>
          </> : <>
            <h1>我的创作</h1>
            <CreationNavigationGuard hasPendingCreation={hasPendingCreation} />
            <form className={styles.createForm} onSubmit={event => { event.preventDefault(); void create(); }}>
              <input aria-label="稿件标题" placeholder="稿件标题" value={title} disabled={creating || unconfirmed} onChange={event => setTitle(event.target.value)} />
              <ActionButton type="submit" variant="primary" disabled={!title.trim() || creating} busy={creating}>{unconfirmed ? '沿用原请求重试' : '新建稿件'}</ActionButton>
            </form>
            <div className={styles.row}>
              <button type="button" aria-pressed={filter === 'mine'} onClick={() => setFilter('mine')}>我的稿件</button>
              <button type="button" aria-pressed={filter === 'team'} onClick={() => setFilter('team')}>团队稿件</button>
            </div>
            {workspace.isPending ? <p role="status">正在读取稿件…</p> : null}
            <div className={styles.workList}>{workspace.data?.works.map(item =>
              <button type="button" key={item.workId} onClick={() => open(item.workId)}>
                {item.title || '未命名稿件'}
                <small>{item.ownership === 'unknown' ? '创建人未知' : item.ownership === 'mine' ? '我创建的稿件' : '团队成员创建'}{item.publishedRevisionId ? ' · 已登记发布' : ''}</small>
              </button>)}</div>
            {workspace.data && !workspace.data.works.length ? <p>这里还没有稿件。</p> : null}
          </>}
        </section>}
    </main>
  </OpsFrame>;
}

// The list and draft editor mount their navigation blockers in separate views.
function CreationNavigationGuard({ hasPendingCreation }: { hasPendingCreation: () => boolean }) {
  const blocker = useBlocker(hasPendingCreation);
  useBeforeUnload(useCallback((event: BeforeUnloadEvent) => {
    if (hasPendingCreation()) event.preventDefault();
  }, [hasPendingCreation]));

  return blocker.state === 'blocked' ? <div role="alert" className={styles.warning}>
    <p>新建稿件尚未确认，请留在当前页面。</p>
    <ActionButton onClick={() => blocker.reset()}>留在我的创作</ActionButton>
  </div> : null;
}

export function MemberDraft({ draft, accounts, refresh }: { draft: Draft; accounts: Workspace['accounts']; refresh: () => void }) {
  const [title, setTitle] = useState(draft.document.title);
  const [body, setBody] = useState(draft.document.paragraphs.map(item => item.text).join('\n\n'));
  const [tags, setTags] = useState(draft.document.tags.join(' '));
  const [account, setAccount] = useState(draft.document.accountRef ?? '');
  const [revision, setRevision] = useState(draft.revisionId);
  const [busy, setBusy] = useState(false);
  const [unknown, setUnknown] = useState(false);
  const [message, setMessage] = useState('');
  const [failed, setFailed] = useState(false);
  const pending = useRef<(Submission & { expectedRevision: string }) | null>(null);
  const inFlight = useRef(false);
  const writableAccounts = accounts.filter(option => option.canPublish);
  const savedAccount = writableAccounts.find(option => option.ref === draft.document.accountRef);
  const readyToPublish = draft.ownership === 'mine' && (draft.editable || !!savedAccount);
  const form = `member-draft-${draft.workId}`;
  const snapshot = JSON.stringify([title, body, tags, account]);
  const [savedSnapshot, setSavedSnapshot] = useState(snapshot);
  const dirty = draft.editable && snapshot !== savedSnapshot;
  const blocker = useBlocker(dirty || busy || unknown);
  useBeforeUnload(useCallback((event: BeforeUnloadEvent) => {
    if (dirty || busy || unknown) event.preventDefault();
  }, [dirty, busy, unknown]));

  async function saveCurrent() {
    if (inFlight.current) throw new Error('保存进行中');
    const document = {
      ...newDocument(title.trim()),
      accountRef: account || null,
      paragraphs: body ? body.split(/\n\n+/).map((text, index) => ({ text, id: draft.document.paragraphs[index]?.id ?? crypto.randomUUID() })) : [],
      tags: tags.split(/[\s，,]+/).filter(Boolean),
    };
    const command = pending.current ?? { requestId: crypto.randomUUID(), expectedRevision: revision, document };
    pending.current = command;
    inFlight.current = true;
    setBusy(true);
    setMessage('');
    try {
      const receipt = await api.save(draft.workId, command);
      pending.current = null;
      setUnknown(false);
      setRevision(receipt.revision.id);
      setSavedSnapshot(snapshot);
      setFailed(false);
      setMessage('稿件已保存。');
      refresh();
      return { revisionId: receipt.revision.id, title: receipt.revision.document.title,
        account: writableAccounts.find(option => option.ref === receipt.revision.document.accountRef) ?? null };
    } catch (error) {
      const answered = error instanceof ApiError && error.answered;
      if (answered) pending.current = null;
      setUnknown(!answered);
      setFailed(true);
      setMessage(errorMessage(error));
      throw error;
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  }
  async function revisionForPublication() {
    if (draft.editable) return saveCurrent();
    return { revisionId: draft.revisionId, title: draft.document.title, account: savedAccount ?? null };
  }

  return <article>
    <h1>{draft.editable ? '编辑稿件' : draft.document.title || '未命名稿件'}</h1>
    {draft.ownership === 'unknown' ? <p>创建人未知</p> : null}
    {blocker.state === 'blocked' ? <div role="alert" className={styles.warning}>
      <p>{busy || unknown ? '稿件保存结果尚未确认，请留在当前页面。' : '稿件有未保存的编辑。'}</p>
      <ActionButton onClick={() => blocker.reset()}>继续编辑</ActionButton>
      {busy || unknown ? null : <ActionButton onClick={() => blocker.proceed()}>丢弃编辑并离开</ActionButton>}
    </div> : null}
    {draft.editable ? <>
      <form id={form} onSubmit={event => { event.preventDefault(); void saveCurrent().catch(() => undefined); }}>
        <fieldset disabled={busy || unknown} className={styles.editingSurface}>
          <label>标题<input value={title} onChange={event => setTitle(event.target.value)} /></label>
          <label>正文<textarea rows={10} value={body} onChange={event => setBody(event.target.value)} /></label>
          <label>话题<input value={tags} onChange={event => setTags(event.target.value)} /></label>
          <label>发布账号<select aria-label="发布账号" value={account} onChange={event => setAccount(event.target.value)}>
            <option value="">未选账号</option>
            {writableAccounts.map(option => <option key={option.ref} value={option.ref}>{option.name}</option>)}
          </select></label>
        </fieldset>
      </form>
    </> : <>
      {draft.restricted ? <p role="status">稿件含有访问受限的内容，已隐藏相关片段。</p> : null}
      {draft.document.paragraphs.map(paragraph => paragraph.text && <p key={paragraph.id}>{paragraph.text}</p>)}
      {draft.document.tags.length ? <p>{draft.document.tags.join(' · ')}</p> : null}
    </>}
    {draft.editable || readyToPublish ? <div className={styles.row}>
      {draft.editable ? <ActionButton form={form} type="submit" variant="primary" busy={busy} disabled={busy || !title.trim()}>{unknown ? '沿用原请求重试' : '保存稿件'}</ActionButton> : null}
      {readyToPublish ? <CreationPublicationButton workId={draft.workId} domainRef={draft.domainRef}
        disabled={draft.editable && (busy || unknown || !title.trim() || !writableAccounts.some(option => option.ref === account))}
        saveCurrent={revisionForPublication} onRegistered={refresh} /> : null}
    </div> : null}
    {message ? <p role={failed ? 'alert' : 'status'}>{message}</p> : null}
    {draft.publications.map(link => <p key={link.linkRef}>{link.available ?
      <Link to={creationPublicationHref(draft.domainRef, link.publicationRef)}>{link.publishedOn ? `发布记录（${link.publishedOn}）` : '查看已关联发布记录'}</Link> :
      '已关联的发布记录当前不可用'}</p>)}
  </article>;
}

function AccountAssets({ accountRef, accounts, memberRef, onChange }: {
  accountRef: string; accounts: Workspace['accounts']; memberRef: string; onChange: (account: string) => void;
}) {
  const assets = useQuery({
    queryKey: ['member-creation', memberRef, 'assets', accountRef], gcTime: 0,
    queryFn: ({ signal }) => api.readAccountAssets(accountRef, signal),
  });
  const relations = { parent: '陪跑营学员家长', coach: '陪跑老师', none: '与机构无关系', unknown: '身份未确认' };
  return <section aria-label="账号资产">
    <h2>账号资产</h2>
    <label>查看账号<select value={accountRef} onChange={event => onChange(event.target.value)}>
      {accounts.map(account => <option value={account.ref} key={account.ref}>{account.name}</option>)}
    </select></label>
    {assets.isPending ? <p role="status">正在读取账号资产…</p> : assets.isError ? <p role="alert">{errorMessage(assets.error)}</p> : <>
      <p>账号身份 · {assets.data.identity ? relations[assets.data.identity.orgRelation] : '未登记'}</p>
      <h3>人设事实</h3>
      {assets.data.facts.filter(fact => fact.text).map(fact => <p key={fact.id}>{fact.key} · {fact.text}</p>)}
      <h3>经历</h3>
      {assets.data.experiences.filter(item => item.body).map(item => <details key={item.id}><summary>{item.title || item.code}</summary><p>{item.body}</p></details>)}
    </>}
  </section>;
}
