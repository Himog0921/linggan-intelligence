import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useSearchParams } from 'react-router';
import { AppShell } from '../../../app/AppShell';
import { ActionButton } from '../../../shared/ActionButton';
import { DecisionForm } from './DecisionForm';
import { CatalogueView, StatisticsView } from './CatalogueView';
import { DossierBlocks } from '../../corpus/evidence/DossierBlocks';
import { fetchPreview, recompute, relevanceLabels, perspectiveLabels, stateLabels, type Preview, type Judgment, type Scenario } from './api';
import styles from './OfflineReview.module.css';

const fields = { title: '标题', body: '正文', tags: '话题标签', ocr: '图片文字', transcript: '音频转录', frame_text: '视频画面文字' };
export function OfflineReviewPage() {
  const query = useQuery({ queryKey: ['ai-offline-preview'], queryFn: ({ signal }) => fetchPreview(signal), retry: false, refetchOnWindowFocus: false });
  return <AppShell active="topic" activeState="离线预演" boundaryLabel="合成数据 · 非实时" crumb={<>主题图谱 / 离线预演</>} meta="">
    <main className={styles.page}><header className={styles.header}><div><h1>方向与解读 · 离线预演</h1><p>合成样本 · 未调用模型 · 未写正式业务结果</p></div><a href="/topics">返回主题图谱</a></header>
      {query.isPending ? <p role="status">正在读取合成样本…</p> : query.isError ? <div role="alert"><p>合成样本读取失败，未采用无法核对的内容。</p><ActionButton onClick={() => void query.refetch()}>重新读取</ActionButton></div> : <LoadedReview initial={query.data} />}
    </main>
  </AppShell>;
}

function LoadedReview({ initial }: { initial: Preview }) {
  const [data, setData] = useState(initial);
  const [formRevision, setFormRevision] = useState(0);
  const [busy, setBusy] = useState(false); const [notice, setNotice] = useState(''); const [error, setError] = useState('');
  const [query, setQuery] = useSearchParams();
  const view = query.get('view') === 'catalogue' ? 'catalogue' : 'notes';
  const ref = query.get('note') ?? data.notes[0]?.input.identity.workRef;
  const note = data.notes.find(n => n.input.identity.workRef === ref);
  const judgments = data.notes.flatMap(n => n.result.judgment ? [n.result.judgment] : []);
  async function calculate(scenario: Scenario, next: Judgment[], message: string, resetForm = false) {
    setBusy(true); setNotice(''); setError('');
    try { setData(await recompute(scenario, next)); if (resetForm) setFormRevision(revision => revision + 1); setNotice(message); }
    catch { setError('预演校验未通过或未收到有效答复；保留上次结果与当前草稿，尚未应用这次修改。'); }
    finally { setBusy(false); }
  }
  function select(workRef: string) { setQuery({ view: 'notes', note: workRef }); }
  const directionName = (direction: string | null, version = data.catalogue.versionRef) => direction === null ? '暂无合适方向' : [data.baselineCatalogue, data.candidateCatalogue].find(c => c.versionRef === version)?.directions.find(d => d.directionRef === direction)?.name ?? '版本中无此方向';
  return <>
    <div className={styles.toolbar}><nav aria-label="离线预演视图">{([['notes', '逐篇核对'], ['catalogue', '方向清单']] as const).map(([key, label]) => <button key={key} type="button" disabled={busy} aria-current={view === key ? 'page' : undefined} onClick={() => setQuery({ view: key, ...(ref ? { note: ref } : {}) })}>{label}</button>)}</nav>
      <div><label htmlFor="offline-scenario">示例版本</label><select id="offline-scenario" value={data.scenario} disabled={busy} onChange={event => { const value = event.target.value; if (value === 'baseline' || value === 'candidate') void calculate(value, judgments, '已切换固定示例版本；没有启用或发布任何版本。'); }}><option value="baseline">旧版示例</option><option value="candidate">候选版示例</option></select></div>
      <ActionButton disabled={busy} onClick={() => void calculate('baseline', [], '已恢复固定示例；本次会话裁定已清除。', true)}>恢复全部示例</ActionButton>
    </div>
    <details className={styles.identity}><summary>固定样本身份与边界</summary><p>样本来源：人工编写的6篇合成材料。没有采样真实库。</p><p>版本：{data.sample.fixtureRevision} · 冻结时间：{data.sample.asOf}</p><p>样本摘要：<code>{data.sample.manifestSha256}</code></p></details>
    {busy ? <p role="status">正在计算本次离线预演…</p> : null}{notice ? <p role="status" className={styles.notice}>{notice}</p> : null}{error ? <p role="alert">{error}</p> : null}
    {view === 'catalogue' ? <CatalogueView data={data} select={select} /> : <div className={styles.workspace}>
      <nav className={styles.index} aria-label="合成笔记">{data.notes.map(item => <button key={item.input.identity.workRef} aria-current={item === note ? 'page' : undefined} type="button" disabled={busy} onClick={() => select(item.input.identity.workRef)}><strong>{item.title}</strong><small>合成点赞：{item.likes === null ? '未知' : item.likes.toLocaleString('zh-CN')}<br />发布周：{item.publishedWeek ?? '未知'}</small>{item.result.needsReview ? <span>待你重看</span> : null}</button>)}</nav>
      {note ? <>
        <section className={styles.reader}><h2>合成输入</h2><p>完整底稿内容；缺失与终态状态保留。</p>{note.input.columns.map(column => <section key={column.field}><h3>{fields[column.field]} <small>{stateLabels[column.state]}</small></h3>{note.input.fragments.filter(f => f.field === column.field).map(fragment => <div key={fragment.citationId}><DossierBlocks blocks={[fragment.structure]} issues={fragment.issues} /><details><summary>片段引用与字符位置</summary><code>{fragment.citationId} · [{fragment.start}, {fragment.end})</code><p>来源 {fragment.sourceRef} · 来源版本 {fragment.sourceVersion} · 冻结时间 {note.input.asOf}</p></details></div>)}</section>)}</section>
        <aside className={styles.result}><h2>固定示例结果</h2><p>离线预演，没有模型分析。</p>
          {note.result.needsReview ? <p role="status" className={styles.notice}><strong>待你重看</strong>：原裁定方向已合并或移除，当前显示机器示例新结果；原裁定仍保留。</p> : note.result.manualApplied ? <p>当前显示本次会话裁定。</p> : <p>当前显示固定机器示例。</p>}
          <dl><dt>相关性</dt><dd>{relevanceLabels[note.result.effective.relevance]}</dd><dt>主方向</dt><dd>{note.result.effective.relevance === 'related' ? directionName(note.result.effective.primaryDirection) : '不归方向'}</dd><dt>也相关（不进统计）</dt><dd>{note.result.effective.alsoRelated ? directionName(note.result.effective.alsoRelated) : '不设置'}</dd><dt>把握 / 谁在讲</dt><dd>{note.result.effective.confidence === 'certain' ? '确定' : '可能'} / {perspectiveLabels[note.result.effective.perspective]}</dd><dt>理由</dt><dd>{note.result.effective.reason ?? '已清除'}</dd></dl>
          <details><summary>关键词字面证据</summary>{note.result.effective.keywords.length ? note.result.effective.keywords.map((k, i) => <p key={i}>{k.text} · <code>{k.citationId} [{k.start}, {k.end})</code></p>) : <p>没有关键词。</p>}</details>
          <details><summary>旧版与当前机器示例对照</summary><p>旧版：{note.baseline.relevance === 'related' ? directionName(note.baseline.primaryDirection, data.baselineCatalogue.versionRef) : '不归方向'}；当前：{note.result.machine.relevance === 'related' ? directionName(note.result.machine.primaryDirection) : '不归方向'}。</p>{note.result.judgment ? <p>保留的会话裁定：{note.result.judgment.relevance === 'related' ? directionName(note.result.judgment.primaryDirection, note.result.judgment.identity.directionVersionRef) : '不归方向'}；理由：{note.result.judgment.reason ?? '已清除'}。</p> : null}</details>
          <DecisionForm key={JSON.stringify([note.input.identity, note.result.judgment, formRevision])} note={note} catalogue={data.catalogue} busy={busy} apply={decision => void calculate(data.scenario, [...judgments.filter(j => j.identity.workRef !== decision.identity.workRef), decision], '本次预演已更新，未保存到业务记录。')} reset={() => void calculate(data.scenario, judgments.filter(j => j.identity.workRef !== note.input.identity.workRef), '已恢复此篇固定示例，未写业务记录。', true)} />
        </aside>
      </> : <p role="alert">这篇笔记不属于固定合成样本，请从左侧重新选择。</p>}
    </div>}
    <StatisticsView data={data} />
  </>;
}
