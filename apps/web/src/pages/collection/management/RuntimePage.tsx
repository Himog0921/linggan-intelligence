import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { CollectionFrame, CollectionReadout } from '../CollectionFrame';
import { CommandForm } from './CommandForm';
import { ManagementReturn } from './ManagementReturn';
import { loadRuntime, type RuntimeProjection, type StationEntry } from './managementApi';
import { useManagementRead } from './useManagementRead';
import { useStationNameOverflow } from './useStationNameOverflow';
import styles from './Management.module.css';

function Truth({ tone, children }: { tone: string; children: ReactNode }) { return <span className={`c-tg-truth ${tone}`}>{children}</span>; }
function SectionHead({ title, meta }: { title: string; meta: ReactNode }) { return <div className="c-sect-head"><h2>{title}</h2><div className="c-sect-meta">{meta}</div></div>; }
function useDialog(ref: React.RefObject<HTMLDialogElement | null>) {
  const [returnFocus] = useState(() => document.activeElement instanceof HTMLElement ? document.activeElement : null);
  useEffect(() => { const element = ref.current; element?.showModal(); return () => { element?.close(); returnFocus?.focus(); }; }, [ref, returnFocus]);
}
function StationManage({ station, accountAvailable, reload, close }: { station: StationEntry; accountAvailable: boolean; reload: () => void; close: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null); useDialog(dialog);
  const account = station.account;
  return <dialog ref={dialog} className="c-stn-modal c-stn-manage-modal" id={`station-manage-${station.stationRef}`} aria-labelledby={`station-manage-title-${station.stationRef}`} onCancel={event => { event.preventDefault(); close(); }}>
    <header className="c-stn-modal-head"><h3 id={`station-manage-title-${station.stationRef}`}>管理 {station.name}</h3><button className="c-btn-quiet c-stn-modal-close" type="button" onClick={close}>关闭</button></header>
    <div className="c-stn-manage-body">
      <div className="c-stn-block"><h3>认领窗口</h3><p className="c-stn-note">{station.claimWindowOpen ? '认领窗口开着：新装的插件会自动归到这台工位。' : '认领窗口已关：新装的插件会停在下面的待认领里等你指认。'}</p>
        <CommandForm action={station.claimWindowOpen ? 'runtime-close-window' : 'runtime-claim-window'} className="c-stn-form" fields={{ station_ref: station.stationRef }} onWritten={reload}>
          {station.claimWindowOpen ? <button className="c-btn-quiet" type="submit">现在就关掉认领窗口</button> : <><label htmlFor={`claim-hours-${station.stationRef}`}>开一个认领窗口</label><select id={`claim-hours-${station.stationRef}`} name="valid_for_hours" defaultValue="24"><option value="24">24 小时</option><option value="8">8 小时</option><option value="1">1 小时</option></select><button className="c-btn-quiet" type="submit">开窗口</button></>}
        </CommandForm>
      </div>
      <div className="c-stn-block"><h3>接活与账号</h3>{account ? <>
        <dl className="c-stn-facts"><div><dt>插件安装</dt><dd>{account.installationLabel}</dd></div><div><dt>服务端凭据</dt><dd>{account.credentialValid ? '有效' : '缺失或已失效'}</dd></div><div><dt>账号绑定</dt><dd>{account.bindingLabel}</dd></div><div><dt>最后一次账号观察</dt><dd>{account.observed ?? '从未观察过'}</dd></div></dl>
        <p className="c-stn-note">{account.hasLiveLease ? '这个账号已经有一份活在跑。' : '这个账号当前没有活在跑。'}账号资格尚未形成结论时按关闭处理，但不会仅因为「很久没观察」就拦住接活（{account.eligibilityReason}）。</p>
        <p className="c-stn-note">{accountAvailable ? '未观察不阻断首单；只有人工确认会替换或结束绑定。' : '本机没有配置账号身份摘要：认证身份不会上报，但明确的登录或限制仍然会阻断接活。'}</p>
        <CommandForm action="runtime-accepting" className="c-stn-form" fields={{ station_ref: station.stationRef, accepting: !account.accepting }} onWritten={reload}><button className="c-btn-quiet" type="submit">{account.accepting ? '暂停这台工位接活' : '恢复这台工位自动接活'}</button></CommandForm>
        {account.installationRef && account.accountRef && ['unconfirmed', 'changed'].includes(account.bindingState) && <CommandForm action="runtime-account-bindings" className="c-stn-form" fields={{ installation_ref: account.installationRef, account_ref: account.accountRef }} onWritten={reload}><button className="c-btn-quiet" type="submit">确认这个观察账号</button></CommandForm>}
      </> : <p className="c-stn-note">这台工位的接活开关与账号绑定当前读不到。这不表示它没有绑定账号。</p>}</div>
      <div className={`c-stn-block${station.capabilities?.length ? ' c-stn-block-wide' : ''}`}><h3>能力{station.capabilities !== null && station.capabilities.length > 0 && <span>近 7 天</span>}</h3>{station.capabilities === null ? <p className="c-stn-note">能力矩阵当前读不到。这不表示这台工位没有能力，只表示这一项此刻答不出。</p> : station.capabilities.length === 0 ? <p className="c-stn-note">这台工位还没有能力记录：在岗插件没有声明能力，近 7 天也没有产出或失败。</p> : <div className="c-caps-grid">{station.capabilities.map(cap => <div className={`c-cap ${cap.className}`} key={cap.capability}><b>{cap.label}</b><span className="c-cap-state">{cap.stateWord}</span><span className="c-cap-detail">{cap.detail}</span>{cap.label !== cap.capability && <code>{cap.capability}</code>}{cap.executionFailures > 0 && <em className="c-cap-hiccup">另有 {cap.executionFailures} 次执行失败（超时或标签页不可用），不是能力问题</em>}</div>)}</div>}</div>
      <div className="c-stn-block"><h3>上次问活</h3><dl className="c-stn-facts"><div><dt>时间</dt><dd>{station.dispatchAt ?? '—'}</dd></div><div><dt>服务端的回答</dt><dd>{station.dispatchLabel}</dd></div></dl><p className="c-stn-note">{station.dispatchHint}</p></div>
      <div className="c-stn-block"><h3>停用</h3><p className="c-stn-note">停用后，这台工位不会再自动领取任务。</p><CommandForm action="runtime-retire" className="c-stn-form" fields={{ station_ref: station.stationRef }} onWritten={reload} onApplied={close}><button className="c-tg-act c-tg-act-danger" type="submit">停用这台工位</button></CommandForm></div>
    </div>
  </dialog>;
}
function StationRow({ station, index, reload, accountAvailable }: { station: StationEntry; index: number; reload: () => void; accountAvailable: boolean }) {
  const [editing, setEditing] = useState(false); const [managing, setManaging] = useState(false);
  const nameInput = useRef<HTMLInputElement>(null); const editButton = useRef<HTMLButtonElement>(null); const wasEditing = useRef(false);
  const nameLabel = useStationNameOverflow(station.name, editing);
  const account = station.account;
  function editName() { if (nameInput.current && nameInput.current.closest("form")?.dataset.commandUnconfirmed !== "true") nameInput.current.value = station.name; setEditing(true); }
  useEffect(() => { if (editing) { nameInput.current?.focus(); nameInput.current?.select(); } else if (wasEditing.current) editButton.current?.focus(); wasEditing.current = editing; }, [editing]);
  function finishEditing() { setEditing(false); }
  return <>
    <div className="c-tg-item c-tg-station-grid c-stn-row" role="row" data-station-entry={station.stationRef}>
      <div className="c-tg-number" role="cell"><span className="c-tg-index">{String(index + 1).padStart(2, '0')}</span></div>
      <div className="c-tg-object c-stn-name-cell" role="cell">
        {!editing && <><span ref={nameLabel} className={`c-tg-title c-stn-name-label ${styles.stationName}`}>{station.name}</span><button ref={editButton} className="c-stn-edit" type="button" onClick={editName} aria-label={`编辑 ${station.name} 工位名称`} title="编辑工位名称">✎</button><button className="c-stn-manage" type="button" onClick={() => setManaging(true)} aria-controls={`station-manage-${station.stationRef}`} aria-label={`管理 ${station.name} 工位`}>管理</button></>}
        <CommandForm action="runtime-rename" className="c-stn-name-form" hidden={!editing} fields={{ station_ref: station.stationRef }} onWritten={reload} onApplied={finishEditing}><input ref={nameInput} className="c-stn-name-input" name="display_name" required maxLength={60} defaultValue={station.name} aria-label="工位名称" onKeyDown={event => { if (event.key === 'Escape') { event.preventDefault(); finishEditing(); } }} /><button className="c-stn-name-save" type="submit">保存</button><button className="c-stn-name-cancel" type="button" onClick={finishEditing} aria-label="取消编辑">×</button></CommandForm>
      </div>
      <div className="c-tg-cell" role="cell"><Truth tone={station.stateTone}>{station.stateLabel}</Truth></div>
      <div className="c-tg-cell c-stn-mono" role="cell">{station.pluginVersion ?? '—'}{station.selectorHealth.map((health, i) => <span className={`c-stn-health ${health.tone}`} title={health.detail} key={i}>{health.label}</span>)}</div>
      <time className="c-tg-cell c-tg-time" role="cell">{station.lastSeen ?? '尚未报到'}</time>
      <div className={`c-tg-cell c-tg-number-value${station.quotaExhausted ? ' c-tg-warn' : ''}`} role="cell"><span className="c-usage c-usage-inline"><span className="c-usage-value">{station.dailyUsed}/{station.dailyQuota}</span><span className="c-usage-track" aria-hidden="true">{Array.from({ length: 12 }, (_, i) => <i key={i} className={`c-usage-seg${i < station.litSegments ? ' on' : ''}`} />)}</span></span></div>
      <div className="c-tg-cell c-stn-accept-cell" role="cell">{account ? <><Truth tone={account.accepting ? 'c-tg-ok' : 'c-tg-neutral'}>{account.accepting ? '自动' : '已暂停'}</Truth><CommandForm action="runtime-accepting" className="c-stn-inline-form" fields={{ station_ref: station.stationRef, accepting: !account.accepting }} onWritten={reload}><button className="c-stn-pause-action" type="submit">{account.accepting ? '暂停' : '恢复'}</button></CommandForm></> : <Truth tone="c-tg-neutral">读不到</Truth>}</div>
      <div className="c-tg-cell" role="cell"><Truth tone={account?.bindingTone ?? 'c-tg-neutral'}>{account?.bindingLabel ?? '读不到'}</Truth></div>
      <div className="c-tg-cell" role="cell"><Truth tone={station.capTone}>{station.capSummary}</Truth></div>
      <div className="c-tg-cell c-stn-recent" role="cell" title={`${station.dispatchLabel} · ${station.dispatchHint}`}>{`${station.dispatchLabel} · ${station.dispatchAt ?? "—"}`}</div>
    </div>
    {managing && <StationManage station={station} accountAvailable={accountAvailable} reload={reload} close={() => setManaging(false)} />}
  </>;
}
function RegisterStation({ close, reload }: { close: () => void; reload: () => void }) {
  const modal = useRef<HTMLElement>(null); const input = useRef<HTMLInputElement>(null);
  const [returnFocus] = useState(() => document.activeElement instanceof HTMLElement ? document.activeElement : null);
  useEffect(() => { input.current?.focus(); return () => returnFocus?.focus(); }, [returnFocus]);
  return <div className="c-stn-overlay" data-station-register-overlay onClick={event => { if (event.target === event.currentTarget) close(); }} onKeyDown={event => {
    if (event.key === 'Escape') { event.preventDefault(); close(); }
    if (event.key !== 'Tab') return;
    const items = modal.current?.querySelectorAll<HTMLElement>('button:not([disabled]),input:not([disabled]),a[href],select:not([disabled])');
    const first = items?.[0]; const last = items?.[items.length - 1];
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
  }}><section ref={modal} className="c-stn-modal" id="station-register" role="dialog" aria-modal="true" aria-labelledby="c-station-register-title"><header className="c-stn-modal-head"><h3 id="c-station-register-title">登记一台工位</h3><button className="c-btn-quiet c-stn-modal-close" type="button" onClick={close}>关闭</button></header><CommandForm action="runtime-register" className="c-stn-form" onWritten={reload} onApplied={close}><label htmlFor="station-name">名字要能让你一眼认出是哪台机器</label><input ref={input} id="station-name" name="display_name" required maxLength={60} defaultValue="本机 Chrome" /><button className="c-btn-primary" type="submit">登记</button></CommandForm><p className="c-stn-note">登记工位、开认领窗口与认领安装都只写本地记录，不访问任何平台。</p></section></div>;
}
function RuntimeDrawer({ data, open, close }: { data: RuntimeProjection; open: boolean; close: () => void }) {
  const closeButton = useRef<HTMLButtonElement>(null);
  useEffect(() => { if (!open) return; const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null; closeButton.current?.focus(); const escape = (event: KeyboardEvent) => { if (event.key === 'Escape') { event.preventDefault(); close(); } }; document.addEventListener('keydown', escape); return () => { document.removeEventListener('keydown', escape); opener?.focus(); }; }, [open, close]);
  const results = data.drawer;
  return <aside className="c-rdrawer" id="c-runtime-drawer" aria-label="运行概览" inert={!open} data-open={open ? '' : undefined}>
    <div className="c-rdrawer-head"><span className="c-rdrawer-title">运行概览</span><button ref={closeButton} className="c-rdrawer-close" type="button" onClick={close} aria-label="关闭运行概览">×</button></div>
    <div className="c-rdrawer-body"><section className="c-rd-sec"><h3 className="c-rd-sec-head">规则策略</h3>{data.lanes === null ? <p className="c-rd-note">通道判定此刻读不到。这不表示没有通道，只表示这一页答不出——两者的处置不同。</p> : <><p className="c-rd-meta">共 {data.lanes.length} 条通道 · {data.lanes.filter(row => row.available).length} 条现在能接活</p>{data.lanes.map(row => <div className="c-rd-rule" key={row.name}><div className="c-rd-rule-head"><b>{row.name}</b><em className={`c-lane-state ${row.className}`}>{row.label}</em>{row.reasonCode && <code className="c-lane-code">{row.reasonCode}</code>}</div><p>{row.needs}</p></div>)}</>}</section>
      <section className="c-rd-sec"><h3 className="c-rd-sec-head">近 7 天执行结果</h3><div className="c-rd-card"><p className="c-rd-card-meta">{results.coverage}</p>{results.readCount === 0 ? <p className="c-rd-note">这一轮没有任何一台工位的能力读数是读到的。这不表示它们没干过活，只表示现在答不出。</p> : results.picked + results.capabilityFailures + results.executionFailures === 0 ? <p className="c-rd-note">这 7 天里，有读数的工位没有留下任何执行记录。</p> : <dl className="c-rd-figures"><div><dt>拿回的采集包</dt><dd>{results.picked}</dd><p>这 7 天从这些工位收上来的包</p></div><div><dt>插件说这项跑不了</dt><dd>{results.capabilityFailures}</dd><p>工位明确回答这项在这里干不了，是能力问题</p></div><div><dt>这一次没跑成</dt><dd>{results.executionFailures}</dd><p>超时、标签页不可用等，能力本身没问题</p></div></dl>}</div></section>
      <section className="c-rd-sec"><h3 className="c-rd-sec-head">补采与失败入口</h3><div className="c-rd-routes"><a className="c-rd-route" href="/collection/targets"><b>补采缺口</b><span>在观察目标页打开目标详情，能看到缺了多少、点一下排入补采</span></a><a className="c-rd-route" href="/collection/tasks"><b>失败与重试</b><span>在任务页看每张工单停在哪一步、上一次为什么没成</span></a></div></section>
      <section className="c-rd-sec"><h3 className="c-rd-sec-head">积压与重试</h3>{results.oldest === null ? <p className="c-rd-note">这一轮没有读到队列事实，因此答不出积压与重试的现状。</p> : <><p className="c-rd-meta">最久在等的一单 · <b>{results.oldest}</b></p>{results.pauses?.length === 0 ? <p className="c-rd-note">当前没有生效中的风险暂停。</p> : <div className="c-rd-pauses">{results.pauses?.map((pause, i) => <div key={i} className="c-rd-pause"><b>{pause.reason}</b><span>由 {pause.by} 于 {pause.at} 暂停</span></div>)}</div>}</>}</section>
    </div><p className="c-rdrawer-foot">{`最近一次派出 ${results.dispatched} · 最近一次拿回 ${results.succeeded}`}</p>
  </aside>;
}
export function RuntimeContent({ data, reload }: { data: RuntimeProjection; reload: () => void }) {
  const [registering, setRegistering] = useState(false); const [drawerOpen, setDrawerOpen] = useState(false);
  const stations = data.stations;
  const container = useRef<HTMLDivElement>(null);
  const sourceFocusApplied = useRef(false);
  useEffect(() => {
    if (sourceFocusApplied.current || !stations) return;
    const selected = new URLSearchParams(window.location.search).get("station");
    if (!selected || !stations.some(station => station.stationRef === selected)) return;
    const row = [...(container.current?.querySelectorAll<HTMLElement>("[data-station-entry]") ?? [])].find(row => row.dataset.stationEntry === selected);
    if (row) { sourceFocusApplied.current = true; row.tabIndex = -1; row.scrollIntoView({ block: "center" }); row.focus(); }
  }, [stations]);
  const closeDrawer = useCallback(() => setDrawerOpen(false), []);
  return <div className="c-runtime" ref={container}>
    <section className={`c-runtime-board${data.lanes === null ? ' c-runtime-board-unknown' : ''} c-verdict-${data.board.tone}`}>
      <div className="c-runtime-board-head"><span className="c-instr-badge">{data.board.badge}</span><strong className="c-runtime-board-state">{data.board.state}</strong><span className="c-runtime-board-code">{data.board.headNote}</span><div className="c-runtime-board-actions"><button className="c-runtime-board-btn c-runtime-board-btn-primary" type="button" onClick={() => setRegistering(true)} aria-haspopup="dialog" aria-expanded={registering} aria-controls="station-register">新增工位</button><button className="c-runtime-board-btn" type="button" onClick={() => setDrawerOpen(open => !open)} aria-controls="c-runtime-drawer" aria-expanded={drawerOpen}>运行概览</button><a className="c-runtime-board-btn" href="/collection/targets">观察目标</a></div></div>
      {data.lanes === null ? <div className="c-runtime-board-empty">现在读不到「能不能接活」的判定。这不表示系统接不了活，只表示这一页此刻答不出。</div> : <div className="c-runtime-board-body"><div className="c-runtime-board-lanes"><div className="c-instr-issue-list">{data.lanes.map(row => <div className="c-instr-issue" data-capacity-state={row.capacityState} key={row.name}><i className="c-lane-mark" aria-hidden="true" /><span className="c-lane-name"><b>{row.name}</b> · {row.detail}</span><em className={`c-lane-state ${row.className}`}>{row.label}</em>{row.reasonCode && <code className="c-lane-code">{row.reasonCode}</code>}</div>)}</div></div><div className="c-runtime-board-metrics">{data.board.metrics.map(metric => <div className="c-runtime-board-metric" key={metric.label}><b>{metric.value}</b><span>{metric.label}</span></div>)}</div></div>}
    </section>
    <section className="c-sect c-stn"><SectionHead title="工位" meta={stations === null ? '台数未知' : `${data.staffedCount} / ${stations.length} 在岗 · 合计 ${stations.length}`} />{stations === null ? <p className="c-stn-empty">工位列表当前读不到。这不表示没有登记工位，也不表示它们离线——两者的处置不同。</p> : stations.length === 0 ? <p className="c-stn-empty">还没有登记任何工位。这不是「等工程」——登记一台是你现在就能做的事，页面下方就是入口。</p> : <div className="c-tg-table-scroll" role="table" aria-label="工位"><div className="c-tg-table-head c-tg-station-grid" role="row">{['编号', '工位', '连接', '工位版本', '心跳时间', '今日额度', '接活', '账号', '能力', '最近动作'].map(label => <span className={label === '今日额度' ? 'c-tg-head-num' : undefined} role="columnheader" key={label}>{label}</span>)}</div><div className="c-stn-list" role="rowgroup">{stations.map((station, index) => <StationRow key={station.stationRef} station={station} index={index} reload={reload} accountAvailable={data.accountObservationAvailable} />)}</div></div>}</section>
    {data.running !== null && <section className="c-sect c-run"><SectionHead title="今天在跑什么" meta={`${data.running.length} 个执行中`} /><div className="c-run-list">{data.running.length === 0 ? <p className="c-stn-empty">当前没有正在执行的任务。</p> : data.running.map((lease, i) => <div className="c-run-row" key={i}><div className="c-run-name"><b>{lease.target}</b><span>{lease.station}</span></div><div className="c-run-meta"><span>已展开成任务</span><span>开始 {lease.started}</span><span>预计 {lease.units} 单元</span><span>到期 {lease.expires}</span></div></div>)}</div></section>}
    {data.schedule !== null && <section className="c-sect c-run"><SectionHead title="自动观察排程" meta={`${data.schedule.length} 条`} />{data.schedule.length === 0 ? <p className="c-stn-empty">当前没有开着的自动观察规则。</p> : <div className="c-tg-table-scroll" role="table" aria-label="自动观察排程"><div className="c-tg-table-head c-sched-grid" role="row">{['目标', '周期', '上次运行', '下次', '操作'].map(label => <span role="columnheader" key={label}>{label}</span>)}</div><div className="c-stn-list" role="rowgroup">{data.schedule.map((rule, i) => <div className="c-tg-item c-sched-grid" role="row" key={i}><div className="c-tg-object" role="cell"><div className="c-tg-object-text"><span className="c-tg-title">{rule.target}</span></div></div><div className="c-tg-cell" role="cell">每 {rule.interval}</div><div className="c-tg-cell c-sched-last" role="cell"><span>{rule.state}</span><time>{rule.last}</time></div><time className="c-tg-cell c-tg-time" role="cell">{rule.next}</time><div className="c-tg-cell c-sched-act" role="cell"><a href="/collection/targets">查看</a></div></div>)}</div></div>}</section>}
    <section className="c-stn-footer">{registering && <RegisterStation close={() => setRegistering(false)} reload={reload} />}{data.unclaimed && data.unclaimed.length > 0 && <details className="c-stn-block c-stn-unclaimed"><summary><b>还没归位的插件安装</b><span>{`${data.unclaimed.filter(row => row.recent).length} 条最近报到 · 共 ${data.unclaimed.length} 条`}</span></summary><p className="c-stn-note">这些插件报到了，但还没人说它们是哪台工位。它们不会被派活。认领窗口开着时新安装会自动归位，关着就停在这里等你指认。</p>{data.unclaimed.every(row => !row.recent) && <p className="c-stn-note">最近 7 天没有新的插件安装在等你指认。</p>}{[true, false].map(recent => <span key={String(recent)}>{!recent && data.unclaimed?.some(row => !row.recent) && <p className="c-stn-note">下面 {data.unclaimed.filter(row => !row.recent).length} 条是更早以前的安装。同一台机器每升级一次插件就会留下一条，它们不会被派活，通常也不需要认领。</p>}{data.unclaimed?.filter(row => row.recent === recent).map(installation => <div className="c-stn-unclaimed-row" data-installation-ref={installation.installationRef} key={installation.installationRef}><div className="c-tg-object-text"><span className="c-tg-title">插件 {installation.pluginVersion}</span><span className="c-tg-meta">{installation.browser} · 首次报到 {installation.firstSeen}</span></div>{stations && stations.length > 0 ? <CommandForm action="runtime-claims" className="c-stn-form" fields={{ installation_ref: installation.installationRef }} onWritten={reload}><select name="station_ref" aria-label="认领到哪台工位">{stations.map(station => <option key={station.stationRef} value={station.stationRef}>{station.name}</option>)}</select><button className="c-btn-quiet" type="submit">认领</button></CommandForm> : <Truth tone="c-tg-neutral">待认领</Truth>}</div>)}</span>)}</details>}</section>
    <RuntimeDrawer data={data} open={drawerOpen} close={closeDrawer} />
  </div>;
}
export function RuntimePage() {
  const read = useManagementRead(loadRuntime);
  return <CollectionFrame section="runtime" meta={<><CollectionReadout value={read.data?.projection?.meta.vacantStations ?? null} label="空缺工位" scope="当前工位投影；空缺不等于离线" /><CollectionReadout value={read.data?.projection?.meta.unclaimedInstallations ?? null} label="未归位安装" scope="当前未认领安装投影" /></>}><ManagementReturn returnTo={read.data?.returnTo ?? '/collection/targets'} runtime />{read.error && <p role="alert">工位列表当前读不到。{read.data ? `以下保留上次读取结果（${read.data.asOf ?? "时刻未知"}）。` : "这不表示没有登记工位。"}<button className="c-btn-quiet" onClick={() => void read.reload()}>重新读取</button></p>}{read.data?.projection ? <RuntimeContent data={read.data.projection} reload={() => void read.reload()} /> : read.loading ? <p role="status">正在读取工位…</p> : <p>工位配置当前读不到；未知状态没有显示为零。</p>}</CollectionFrame>;
}
