import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState, type FormEvent } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { CrIcon } from './CrIcon';
import { studyTime } from './format';
import { StudyDialog } from './StudyDialog';
import type { StudyNavigation } from './studyApi';
import { c } from './styles';
import { changeAutomation, readAutomation, readAutomationSelection, uncertain, why, type CommentAutomationChange, type CommentAutomationSetting } from './understandingApi';

const asTime = (minute: number) => [Math.floor(minute / 60), minute % 60].map(value => String(value).padStart(2, '0')).join(':');

function AutomationForm({ current, saved, reload, close }: {
  current: CommentAutomationSetting; saved: () => void; reload: () => void; close: () => void;
}) {
  const [enabled, setEnabled] = useState(current.enabled);
  const [autoSubmit, setAutoSubmit] = useState(current.autoSubmit);
  const [time, setTime] = useState(asTime(current.minuteOfDay));
  const [timeZone, setTimeZone] = useState(current.timeZone);
  const [limit, setLimit] = useState(String(current.selectionLimit));
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const [retry, setRetry] = useState<CommentAutomationChange | null>(null);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (busy) return;
    const [hour = 0, minute = 0] = time.split(':').map(Number);
    const change = retry ?? {
      commandRef: crypto.randomUUID(), domainRef: current.domainRef, expectedRevision: current.revision,
      enabled, autoSubmit: enabled && autoSubmit, minuteOfDay: hour * 60 + minute, timeZone: timeZone.trim(), selectionLimit: Number(limit),
    };
    setBusy(true); setNotice('');
    try {
      await changeAutomation(change);
      saved();
    } catch (error) {
      if (uncertain(error)) {
        setRetry(change);
        setNotice('保存回执未确认。重试会沿用同一请求。');
      } else {
        setRetry(null);
        setNotice('未能保存。请检查输入，或重新读取当前设置后再试。');
      }
    } finally { setBusy(false); }
  }

  return <form onSubmit={event => void save(event)}>
    <div className={c('automation-form')}>
      <p>启用后，每天按首次接纳时间从早到晚选取本领域的新增评论。超出本批上限的评论留到下一次。</p>
      <fieldset disabled={busy || retry !== null}>
        <label className={c('check')}><input type="checkbox" checked={enabled} onChange={event => setEnabled(event.target.checked)} /> 启用定时生成清单</label>
        <label className={c('check')}><input type="checkbox" checked={enabled && autoSubmit} disabled={!enabled} onChange={event => setAutoSubmit(event.target.checked)} /> 自动提交符合条件的评论</label>
        <label>每天生成时间<input type="time" required value={time} onChange={event => setTime(event.target.value)} /></label>
        <label>时区<input required maxLength={128} value={timeZone} onChange={event => setTimeZone(event.target.value)} placeholder="Asia/Shanghai" /></label>
        <label>每份清单最多评论数<input type="number" required min={1} max={1000} step={1} value={limit} onChange={event => setLimit(event.target.value)} /></label>
      </fieldset>
      <p>{enabled && autoSubmit
        ? '保存后，新清单中的合格评论会使用本领域已授权的材料与任务模型自动提交。缺少作品底稿的评论将在条件满足后接续。'
        : '清单可在研究记录中预览并手动提交。'}</p>
      {notice ? <p role="alert" className={c('inline-alert')}>{notice}</p> : null}
    </div>
    <footer className={c('dialog-footer')}><span />
      <div className={c('actions')}>
        {notice && !retry ? <ActionButton disabled={busy} onClick={reload}>重新读取设置</ActionButton> : null}
        <ActionButton disabled={busy} onClick={close}>取消</ActionButton>
        <ActionButton type="submit" variant="ink" disabled={busy}>{retry ? '重试保存' : '保存设置'}</ActionButton>
      </div>
    </footer>
  </form>;
}

const submissionLabels = {
  pending: '等待提交',
  waiting_l1: '等待作品底稿',
  admission_blocked: '提交条件未满足',
  skipped: '已跳过',
  submitted: '已提交',
};

function submissionSummary(counts: Record<string, unknown>) {
  return Object.entries(submissionLabels).flatMap(([key, label]) => {
    const count = counts[key];
    return typeof count === 'number' && count > 0 ? [`${label} ${count} 条`] : [];
  }).join(' · ');
}
export function CommentAutomation({ domain, nav, showingSettings, closeSettings }: {
  domain: string; nav: StudyNavigation; showingSettings: boolean; closeSettings: () => void;
}) {
  const queryClient = useQueryClient();
  const [before, setBefore] = useState<string | null>(null);
  const [opening, setOpening] = useState<string | null>(null);
  const [notice, setNotice] = useState('');
  const automation = useQuery({
    queryKey: ['comment-understanding', domain, 'automation', before],
    queryFn: ({ signal }) => readAutomation(domain, before, signal), retry: false,
    refetchInterval: query => !before && query.state.data?.setting.enabled ? 15000 : false,
  });
  async function preview(selectionRef: string) {
    setOpening(selectionRef); setNotice('');
    try {
      // Always re-read: a comment can be re-observed under a new source between generation and preview.
      const selected = await queryClient.fetchQuery({ queryKey: ['comment-understanding', domain, 'automation-selection', selectionRef], staleTime: 0, gcTime: 0,
        queryFn: ({ signal }) => readAutomationSelection(domain, selectionRef, signal) });
      if (selected.length) nav.preview(selected); else setNotice('清单内暂时没有可读取的评论。');
    } catch { setNotice('清单读取失败，请重试。'); }
    finally { setOpening(null); }
  }
  const setting = automation.data?.setting;
  const nextBefore = automation.data?.nextBefore;
  const settingsTitle = 'comment-automation-title';

  return <>
    {setting && (setting.enabled || automation.data?.selections.length || before) ? <section aria-label="自动研究清单">
      <h3>自动研究清单</h3>
      <p>{setting.enabled ? `每天 ${asTime(setting.minuteOfDay)} · ${setting.timeZone} · 每批最多 ${setting.selectionLimit} 条${setting.autoSubmit ? " · 自动提交已开启" : ""}` : '已暂停生成新清单'}</p>
      {automation.data?.selections.length ? <ul>{automation.data.selections.map(item => {
        const submission = submissionSummary(item.submissionCounts);
        const reasons = [...new Set(item.submissionReasons.flatMap(value => value.split(',')).map(why))];
        return <li key={item.selectionRef}>
        <span>{studyTime(item.generatedAt)} · {item.targetCount ? `${item.targetCount} 条评论` : '无符合条件的新增评论'}</span>
        {submission ? <span>{submission}</span> : null}
        {reasons.length ? <span>{reasons.join('、')}</span> : null}
        {item.batchRefs.map(batch => <button type="button" key={batch} className={c('text-action')} onClick={() => nav.records(batch)}>
          打开批次 {batch.slice(0, 8)}
        </button>)}
        {item.targetCount ? <button type="button" className={c('text-action')} disabled={opening !== null} onClick={() => void preview(item.selectionRef)}>{opening === item.selectionRef ? '正在读取清单…' : '预览清单'} <CrIcon name="ArrowRight" /></button> : null}
      </li>; })}</ul> : <p>暂无自动研究清单。</p>}
      {before || nextBefore ? <div className={c('records-detail-actions')}>
        {before ? <ActionButton className={c('small')} onClick={() => setBefore(null)}>最新清单</ActionButton> : null}
        {nextBefore ? <ActionButton className={c('small')} onClick={() => setBefore(nextBefore)}>更早清单</ActionButton> : null}
      </div> : null}
      {notice ? <p role="status">{notice}</p> : null}
    </section> : notice ? <p role="status">{notice}</p> : null}
    {showingSettings ? <StudyDialog titleId={settingsTitle} onClose={closeSettings} className={c('dialog automation-dialog')}>
      <header className={c('dialog-head')}><h2 id={settingsTitle}>评论研究自动化设置</h2>
        <button type="button" className={c('icon-btn')} aria-label="关闭自动化设置" onClick={closeSettings}><CrIcon name="X" /></button></header>
      {automation.isPending ? <p className={c('automation-form')}>正在读取自动化设置…</p> : automation.isError ? <div className={c('automation-form')}>
        <p role="alert">自动化设置读取失败。</p>
        <ActionButton onClick={() => void automation.refetch()}>重新读取设置</ActionButton>
      </div> : setting ? <AutomationForm key={setting.revision} current={setting} close={closeSettings}
        reload={() => void automation.refetch()} saved={() => { closeSettings(); nav.toast('自动化设置已保存。'); void automation.refetch(); }} /> : null}
    </StudyDialog> : null}
  </>;
}
