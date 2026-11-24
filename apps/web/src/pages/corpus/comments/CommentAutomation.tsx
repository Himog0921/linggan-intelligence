import { skipToken, useQuery } from '@tanstack/react-query';
import { useState, type FormEvent } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { CommentResearchSelection } from './CommentResearchSelection';
import { studyTime } from './format';
import { StudyDialog } from './StudyDialog';
import type { StudyNavigation } from './studyApi';
import { c } from './styles';
import { changeAutomation, readAutomation, readAutomationSelection, uncertain, type CommentAutomationChange, type CommentAutomationSetting } from './understandingApi';

const asTime = (minute: number) => [Math.floor(minute / 60), minute % 60].map(value => String(value).padStart(2, '0')).join(':');

function AutomationForm({ current, saved, reload, close }: {
  current: CommentAutomationSetting; saved: () => void; reload: () => void; close: () => void;
}) {
  const [enabled, setEnabled] = useState(current.enabled);
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
      enabled, minuteOfDay: hour * 60 + minute, timeZone: timeZone.trim(), selectionLimit: Number(limit),
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

  return <form className={c('study-detail-section')} onSubmit={event => void save(event)}>
    <p>启用后，每天按首次接纳时间从早到晚选取本领域的新增评论。超出本批上限的评论留到下一次。</p>
    <fieldset className={c('study-automation-fields')} disabled={busy || retry !== null}>
      <label><input type="checkbox" checked={enabled} onChange={event => setEnabled(event.target.checked)} /> 启用定时生成清单</label>
      <label>每天生成时间<input type="time" required value={time} onChange={event => setTime(event.target.value)} /></label>
      <label>时区<input required maxLength={128} value={timeZone} onChange={event => setTimeZone(event.target.value)} placeholder="Asia/Shanghai" /></label>
      <label>每份清单最多评论数<input type="number" required min={1} max={1000} step={1} value={limit} onChange={event => setLimit(event.target.value)} /></label>
    </fieldset>
    <p>生成清单后，从研究记录打开并预览，再由运营确认启动。</p>
    {notice ? <p role="alert">{notice}</p> : null}
    <div className={c('study-link-actions')}>
      <ActionButton type="submit" variant="primary" disabled={busy}>{retry ? '重试保存' : '保存设置'}</ActionButton>
      {notice && !retry ? <ActionButton type="button" disabled={busy} onClick={reload}>重新读取设置</ActionButton> : null}
      <ActionButton type="button" disabled={busy} onClick={close}>取消</ActionButton>
    </div>
  </form>;
}

export function CommentAutomation({ domain, nav, showingSettings, closeSettings }: {
  domain: string; nav: StudyNavigation; showingSettings: boolean; closeSettings: () => void;
}) {
  const [before, setBefore] = useState<string | null>(null);
  const [selectedRef, setSelectedRef] = useState<string | null>(null);
  const [notice, setNotice] = useState('');
  const automation = useQuery({
    queryKey: ['comment-understanding', domain, 'automation', before],
    queryFn: ({ signal }) => readAutomation(domain, before, signal), retry: false,
    refetchInterval: query => !before && query.state.data?.setting.enabled ? 15000 : false,
  });
  const selection = useQuery({
    queryKey: ['comment-understanding', domain, 'automation-selection', selectedRef],
    queryFn: selectedRef === null ? skipToken : ({ signal }) => readAutomationSelection(domain, selectedRef, signal),
    retry: false, gcTime: 0, refetchOnWindowFocus: false, refetchOnReconnect: false,
  });
  const sourceKey = selection.data?.map(comment => comment.sourceRef).join(',');
  const setting = automation.data?.setting;
  const nextBefore = automation.data?.nextBefore;
  const settingsTitle = 'comment-automation-title';

  return <>
    {setting && (setting.enabled || automation.data?.selections.length || before) ? <section className={c('study-detail-section')} aria-label="自动研究清单">
      <h3>自动研究清单</h3>
      <p>{setting.enabled ? `每天 ${asTime(setting.minuteOfDay)} · ${setting.timeZone} · 每批最多 ${setting.selectionLimit} 条` : '已暂停生成新清单'}</p>
      {automation.data?.selections.length ? <ul>{automation.data.selections.map(item => <li key={item.selectionRef}>
        {studyTime(item.generatedAt)} · {item.targetCount ? `${item.targetCount} 条评论` : '无符合条件的新增评论'}
        {item.targetCount ? <> · <button type="button" className={c('study-link')} aria-pressed={selectedRef === item.selectionRef} onClick={() => {
          if (selectedRef === item.selectionRef) void selection.refetch();
          else setSelectedRef(item.selectionRef);
        }}>{selectedRef === item.selectionRef ? '刷新清单' : '查看清单'}</button></> : null}
      </li>)}</ul> : <p>暂无自动研究清单。</p>}
      <div className={c('study-link-actions')}>
        {before ? <ActionButton type="button" onClick={() => { setBefore(null); setSelectedRef(null); }}>最新清单</ActionButton> : null}
        {nextBefore ? <ActionButton type="button" onClick={() => { setBefore(nextBefore); setSelectedRef(null); }}>更早清单</ActionButton> : null}
      </div>
      {selectedRef && selection.isPending ? <p>正在读取清单…</p> : null}
      {selectedRef && selection.isError ? <p role="alert">清单读取失败。 <button type="button" onClick={() => void selection.refetch()}>重试</button></p> : null}
      {selectedRef && selection.data && selection.data.length > 0 ? <CommentResearchSelection key={`${selectedRef}:${sourceKey}`} domain={domain} selected={selection.data} nav={nav} /> : null}
      {selectedRef && selection.data?.length === 0 ? <p>清单内暂时没有可读取的评论。</p> : null}
    </section> : null}
    {notice ? <p role="status">{notice}</p> : null}
    {showingSettings ? <StudyDialog titleId={settingsTitle} onClose={closeSettings} className={c('study-detail-dialog')}>
      <header className={c('study-dialog-header')}><h2 id={settingsTitle}>评论研究自动化设置</h2></header>
      {automation.isPending ? <p>正在读取自动化设置…</p> : automation.isError ? <div className={c('study-detail-section')}>
        <p role="alert">自动化设置读取失败。</p>
        <ActionButton type="button" onClick={() => void automation.refetch()}>重新读取设置</ActionButton>
      </div> : setting ? <AutomationForm key={setting.revision} current={setting} close={closeSettings}
        reload={() => void automation.refetch()} saved={() => { closeSettings(); setNotice('自动化设置已保存。'); void automation.refetch(); }} /> : null}
    </StudyDialog> : null}
  </>;
}
