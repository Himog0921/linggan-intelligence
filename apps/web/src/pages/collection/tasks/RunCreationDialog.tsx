import { useQuery } from '@tanstack/react-query';
import { useEffect, useRef, useState, type FormEvent } from 'react';
import { RunDialog } from './RunDialog';
import { createRun, definiteRejection, type RunRequest } from './runRequests';
import { errorText, object, readDomains, readTargets, uuid } from './tasksApi';

interface Draft { domain: string; source: string; query: string; target: string; ranking: string; workLimit: string; commentMode: string }
function restored(key: string, params: URLSearchParams): { draft: Draft; request: RunRequest | null } {
  const fallback: Draft = { domain: params.get('domain') === 'all' ? '' : params.get('domain') ?? '', source: params.has('target') ? 'observation_target' : 'temporary_keyword', query: '', target: params.get('target') ?? '', ranking: 'comprehensive', workLimit: '50', commentMode: 'sample_30' };
  try {
    const v = JSON.parse(sessionStorage.getItem(key) ?? 'null');
    if (!v || !v.draft || Object.keys(fallback).some(k => typeof v.draft[k] !== 'string')) return { draft: fallback, request: null };
    const r = v.request;
    return { draft: v.draft, request: r && typeof r.fingerprint === 'string' && typeof r.sent === 'boolean' ? { identity: uuid(r.identity), fingerprint: r.fingerprint, sent: r.sent } : null };
  } catch { return { draft: fallback, request: null }; }
}
function selectedWorks(params: URLSearchParams): string[] | null {
  if (params.get('selected') !== '1') return null;
  try { const items: unknown = JSON.parse(sessionStorage.getItem(`collection-selected:${params.get('domain') ?? ''}`) ?? '[]'); return Array.isArray(items) ? items.map(uuid) : []; } catch { return []; }
}
export function RunCreationDialog({ params, onClose, onCreated }: { params: URLSearchParams; onClose: () => void; onCreated: (receipt: { runRef: string; domainRef: string }) => void }) {
  const [exactWorks] = useState(() => selectedWorks(params));
  const draftKey = `collection-run-draft:${params.get('domain') ?? 'all'}${exactWorks === null ? params.has('target') ? `:target:${params.get('target')}` : '' : `:selected:${exactWorks.join(',')}`}`;
  const [initial] = useState(() => restored(draftKey, params));
  const [draft, setDraft] = useState(initial.draft);
  const attempt = useRef<RunRequest | null>(initial.request);
  const sending = useRef(false);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const [busy, setBusy] = useState(false);
  const [uncertain, setUncertain] = useState(initial.request?.sent ?? false);
  const [feedback, setFeedback] = useState('');
  const [search, setSearch] = useState('');
  const [term, setTerm] = useState('');
  const [targetPage, setTargetPage] = useState(1);
  const [priorTargets, setPriorTargets] = useState<Awaited<ReturnType<typeof readTargets>>['items']>([]);
  const domains = useQuery({ queryKey: ['run-domains'], queryFn: ({ signal }) => readDomains(signal), gcTime: 0 });
  const targets = useQuery({ queryKey: ['run-targets', draft.domain, term, targetPage, draft.target], queryFn: ({ signal }) => readTargets(draft.domain, term, targetPage, draft.target, signal), enabled: draft.source === 'observation_target' && !!draft.domain && exactWorks === null, gcTime: 0 });
  useEffect(() => { const timer = setTimeout(() => setTerm(search), 250); return () => clearTimeout(timer); }, [search]);
  useEffect(() => { try { sessionStorage.setItem(draftKey, JSON.stringify({ draft, request: attempt.current })); } catch { /* input remains in the open dialog */ } }, [draft, draftKey]);
  const options = [...priorTargets, ...(targets.data?.items ?? [])].filter((t, i, all) => all.findIndex(v => v.target_ref === t.target_ref) === i);
  const creator = draft.source === 'observation_target' && options.find(t => t.target_ref === draft.target)?.target_kind === 'creator';
  function update(key: keyof Draft, value: string) { if (sending.current || uncertain) return; attempt.current = null; setDraft(v => ({ ...v, [key]: value })); if (key === 'domain' || key === 'source') { setTargetPage(1); setPriorTargets([]); } }
  function persist() { try { sessionStorage.setItem(draftKey, JSON.stringify({ draft, request: attempt.current })); } catch { /* no storage is required to hold an in-memory request */ } }
  async function submit(event: FormEvent) {
    event.preventDefault(); if (sending.current) return;
    if (exactWorks !== null && !exactWorks.length) { setFeedback('所选作品集合无效，请回到语料重新选择。'); return; }
    let intent: Record<string, unknown> = { domainRef: draft.domain, source: exactWorks !== null ? { kind: 'selected_works', workRefs: exactWorks } : draft.source === 'observation_target' ? { kind: 'observation_target', targetRef: draft.target } : { kind: 'temporary_keyword', query: draft.query }, ranking: creator || exactWorks !== null ? null : draft.ranking, workLimit: exactWorks?.length ?? Number(draft.workLimit), commentMode: exactWorks !== null ? 'all_public' : draft.commentMode };
    if (attempt.current?.sent) {
      try {
        const original = object(JSON.parse(attempt.current.fingerprint));
        if (original.domainRef !== draft.domain) throw new Error('scope mismatch');
        intent = original;
      } catch { setFeedback('原请求范围无法核对，已保留原身份；当前不能重新提交。'); return; }
    }
    attempt.current ??= { identity: crypto.randomUUID(), fingerprint: JSON.stringify(intent), sent: false };
    const wasSent = attempt.current.sent;
    sending.current = true; setBusy(true); setFeedback(wasSent ? '正在核对原请求回执…' : '正在建立这次请求…');
    try {
      const pending = createRun(attempt.current, intent); persist();
      const receipt = await pending;
      // Closing a pending dialog keeps its identity available for a later receipt read.
      // A late answer must not reopen a drawer or navigate a different current page.
      if (mounted.current) { sessionStorage.removeItem(draftKey); onCreated(receipt); }
    } catch (error) {
      if (!wasSent && definiteRejection(error)) { attempt.current = null; setUncertain(false); setFeedback(`请求未被接受：${errorText(error)}。草稿已保留。`); }
      else { setUncertain(true); setFeedback(`请求未确认：${errorText(error)}。草稿及申请身份已保留，重试先核对原回执。`); }
      persist();
    } finally { sending.current = false; setBusy(false); }
  }
  return <RunDialog id="collection-run-form" labelledBy="collection-run-form-title" onClose={onClose}><form data-create-run onSubmit={submit}><header><div><h2 id="collection-run-form-title">新建单次采集</h2><p>为当前问题取得一批材料，结束后不自动重复。</p></div><button className="c-btn-quiet" type="button" onClick={onClose} aria-label="关闭单次采集">×</button></header><div className="c-run-dialog-body">
    {exactWorks !== null && <p>已选择 {exactWorks.length} 篇作品；沿用精确 ID，不重新搜索。</p>}
    <fieldset hidden={exactWorks !== null} disabled={busy || uncertain}><legend>采集来源</legend><div className="c-run-source">{([['temporary_keyword', '临时关键词'], ['observation_target', '已有观察目标']] as const).map(([value, label]) => <label key={value}><input type="radio" name="source" value={value} checked={draft.source === value} onChange={() => update('source', value)} />{label}</label>)}</div></fieldset>
    <label>本次归属领域 *<select name="domain" required value={draft.domain} disabled={busy || uncertain} onChange={e => update('domain', e.target.value)}><option value="">请选择本次领域</option>{domains.data?.filter(d => d.status === 'active').map(d => <option key={d.domainRef} value={d.domainRef}>{d.name}</option>)}</select></label>
    <label data-keyword-field hidden={exactWorks !== null || draft.source === 'observation_target'}>这次想找什么 *<input name="query" maxLength={80} placeholder="例如：对立违抗" required={exactWorks === null && draft.source !== 'observation_target'} value={draft.query} disabled={busy || uncertain} onChange={e => update('query', e.target.value)} /></label>
    <section data-target-field hidden={exactWorks !== null || draft.source !== 'observation_target'}><label>搜索本领域观察目标<input type="search" name="target_search" value={search} disabled={busy || uncertain} onChange={e => { setSearch(e.target.value); setTargetPage(1); setPriorTargets([]); }} /></label><select name="target" aria-label="选择观察目标" value={draft.target} required={exactWorks === null && draft.source === 'observation_target'} disabled={busy || uncertain} onChange={e => update('target', e.target.value)}><option value="">{draft.domain ? '请选择观察目标' : '请先选择本次领域'}</option>{options.map(t => <option key={t.target_ref} value={t.target_ref}>{t.name} · {t.target_kind === 'creator' ? '创作者' : '关键词'}</option>)}</select><button type="button" className="c-btn-quiet" disabled={busy || uncertain || targets.isFetching || !targets.data || targetPage * 20 >= targets.data.total} onClick={() => { setPriorTargets(options); setTargetPage(targetPage + 1); }}>加载下一页</button></section>
    <label data-ranking-field hidden={creator || exactWorks !== null}>搜索排序<select name="ranking" value={draft.ranking} disabled={busy || uncertain} onChange={e => update('ranking', e.target.value)}>{[['comprehensive', '综合'], ['latest', '最新'], ['most_liked', '最多点赞'], ['most_collected', '最多收藏'], ['most_commented', '最多评论']].map(([v, label]) => <option key={v} value={v}>{label}</option>)}</select></label>
    <label><span>{exactWorks !== null ? '本次作品数量' : '发现作品上限'}</span><input name="work_limit" type="number" value={exactWorks?.length ?? draft.workLimit} min={1} max={200} required readOnly={exactWorks !== null} disabled={busy || uncertain} onChange={e => update('workLimit', e.target.value)} /><small>{exactWorks !== null ? '沿用已勾选的作品，不重新搜索或补换作品。' : '1–200 篇；实际发现可能更少，已有作品仍占本次位置。'}</small></label>
    <label>评论范围<select name="comment_mode" value={exactWorks !== null ? 'all_public' : draft.commentMode} disabled={busy || uncertain || exactWorks !== null} onChange={e => update('commentMode', e.target.value)}><option value="sample_30">每篇最多 30 条评论样本，含回复</option><option value="all_public">尽量采全，含回复</option></select></label><p>临时采集不加入长期名单；本次参数不修改已有目标的周期规则。</p><p role="alert" data-run-form-feedback>{feedback || (domains.error ? `领域读取失败：${errorText(domains.error)}` : targets.error ? `目标读取失败：${errorText(targets.error)}。草稿已保留。` : '')}</p></div><footer><button type="button" className="c-btn-quiet" onClick={onClose}>取消</button><button type="submit" className="c-btn-primary c-run-primary" disabled={busy || (!uncertain && domains.isPending)}>{uncertain ? '核实原请求回执' : '▷ 发起采集'}</button></footer></form></RunDialog>;
}
