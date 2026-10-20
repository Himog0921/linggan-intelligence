import { useEffect, useRef, useState } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { CommandForm } from './CommandForm';
import { RegisterStation, StationSettings } from './StationManagement';
import type { RuntimeProjection } from './managementApi';
import styles from './Management.module.css';

type View = 'stations' | 'running' | 'schedule' | 'claims';
const views = [['stations','工位'],['running','今天在跑什么'],['schedule','自动观察排程'],['claims','插件认领']] as const;
export function RuntimeContent({ data, reload }: { data: RuntimeProjection; reload: () => void }) {
  const [view, setView] = useState<View>('stations');
  const [registering, setRegistering] = useState(false);
  const [expanded, setExpanded] = useState<string | null>(() => new URLSearchParams(window.location.search).get('station'));
  const container = useRef<HTMLDivElement>(null);
  const stations = data.stations;
  const sourceFocusApplied=useRef(false);
  useEffect(() => {
    if(sourceFocusApplied.current || !stations) return;
    const selected=new URLSearchParams(window.location.search).get('station');
    const row=[...(container.current?.querySelectorAll<HTMLElement>('[data-station-entry]') ?? [])].find(item=>item.dataset.stationEntry===selected);
    if(row){sourceFocusApplied.current=true;row.tabIndex=-1;row.focus({preventScroll:true});row.scrollIntoView?.({block:'nearest'});}
  },[stations]);
  const counts = {stations:stations?.length,running:data.running?.length,schedule:data.schedule?.length,claims:data.unclaimed?.filter(i=>i.recent).length};
  return <div className={styles.workspace} ref={container}>
    <div className={styles.controls}><div className={styles.tools}><span className={styles.scope}>{stations === null ? '工位数量未知' : `${data.staffedCount ?? '—'} 台已有插件 · 共 ${stations.length} 台`}</span><div className={styles.actions}><ActionButton variant="quiet" onClick={reload}>刷新</ActionButton><ActionButton variant="primary" onClick={() => {setRegistering(true);setView('stations');}} aria-expanded={registering}>＋ 新增工位</ActionButton></div></div>
    <nav className={styles.tabs} aria-label="工位管理内容">{views.map(([key,label])=><button type="button" key={key} aria-current={view===key?'page':undefined} onClick={()=>setView(key)}>{label}<span>{counts[key] ?? '—'}</span></button>)}</nav></div>
    <div hidden={!registering}><RegisterStation reload={reload} onApplied={()=>setRegistering(false)}/><ActionButton variant="quiet" onClick={()=>setRegistering(false)}>收起新增表单，保留输入</ActionButton></div>
    <section hidden={view!=='stations'} aria-label="工位列表">{stations === null ? <p className={styles.empty}>工位列表当前读不到，不能据此认定没有工位或全部离线。</p> : stations.length === 0 ? <p className={styles.empty}>尚未登记工位，使用“新增工位”连接执行设备。</p> : stations.map((station,index)=><article key={station.stationRef} data-station-entry={station.stationRef} data-expanded={expanded===station.stationRef} className={styles.station}>
      <div className={styles.stationHead}><span className={styles.index}>{String(index+1).padStart(2,'0')}</span><h3>{station.name}</h3><span className={`c-tg-truth ${station.stateTone}`}>{station.stateLabel}</span><button type="button" className={styles.manage} aria-expanded={expanded===station.stationRef} aria-controls={`station-settings-${station.stationRef}`} aria-label={`管理 ${station.name} 工位`} onClick={()=>setExpanded(expanded===station.stationRef?null:station.stationRef)}>能力与设置 {expanded===station.stationRef?'−':'＋'}</button></div>
      <dl className={styles.stationFacts}><div><dt>插件版本</dt><dd>{station.pluginVersion ?? '—'}{station.selectorHealth.map((health,i)=><span className={`c-tg-truth ${health.tone}`} title={health.detail} key={i}>{health.label}</span>)}</dd></div><div><dt>最近报到</dt><dd>{station.lastSeen ?? '尚未报到'}</dd></div><div><dt>今日额度</dt><dd>{station.dailyUsed}/{station.dailyQuota}</dd></div><div><dt>接活</dt><dd>{station.account ? station.account.accepting ? '自动' : '已暂停' : '未知'}</dd></div><div><dt>账号</dt><dd>{station.account?.bindingLabel ?? '未知'}</dd></div><div><dt>能力</dt><dd className={`c-tg-truth ${station.capTone}`}>{station.capSummary}</dd></div></dl>
      <p className={styles.recent}>{station.dispatchLabel} · {station.dispatchAt ?? '时间未知'}</p>
      <div id={`station-settings-${station.stationRef}`} hidden={expanded!==station.stationRef}><StationSettings station={station} accountAvailable={data.accountObservationAvailable} reload={reload}/></div>
    </article>)}</section>
    <section hidden={view!=='running'} aria-label="今天在跑什么">{data.running === null ? <p className={styles.empty}>当前执行情况读不到。</p> : data.running.length===0 ? <p className={styles.empty}>当前没有正在执行的任务。</p> : data.running.map((run,index)=><article className={styles.execution} key={index}><div><h3>{run.target}</h3><span>{run.station}</span></div><dl className={styles.facts}><div><dt>开始</dt><dd>{run.started}</dd></div><div><dt>执行期限</dt><dd>{run.expires}</dd></div><div><dt>预计执行量</dt><dd>{run.units} 单元</dd></div></dl></article>)}</section>
    <section hidden={view!=='schedule'} aria-label="自动观察排程">{data.schedule===null ? <p className={styles.empty}>自动观察排程当前读不到。</p> : data.schedule.length===0 ? <p className={styles.empty}>当前没有开启的自动观察规则。</p> : <div className={styles.scheduleScroll}><table className={styles.schedule}><thead><tr>{['目标','周期','上次运行','下次运行','操作'].map(label=><th key={label}>{label}</th>)}</tr></thead><tbody>{data.schedule.map((rule,index)=><tr key={index}><td>{rule.target}</td><td>每 {rule.interval}</td><td>{rule.state}<br/>{rule.last}</td><td>{rule.next}</td><td><a href="/collection/targets">查看目标</a></td></tr>)}</tbody></table></div>}</section>
    <section hidden={view!=='claims'} aria-label="插件认领"><p className={styles.help}>选择插件对应的工位，认领后可接收任务。新设备请先新增工位。</p>{data.unclaimed===null ? <p className={styles.empty}>待认领安装当前读不到。</p> : <>{data.unclaimed.some(i=>i.recent) && <h3 className={styles.groupTitle}>最近报到</h3>}{data.unclaimed.some(i=>i.recent) ? data.unclaimed.filter(i=>i.recent).map(installation=><Installation key={installation.installationRef} installation={installation} stations={stations} reload={reload}/>) : <p className={styles.empty}>最近 7 天没有待认领的插件安装。</p>}{data.unclaimed.some(i=>!i.recent) && <details className={styles.history}><summary>较早的安装 · {data.unclaimed.filter(i=>!i.recent).length} 条</summary><p className={styles.help}>历史安装不接收任务，确认仍在使用后再认领。</p>{data.unclaimed.filter(i=>!i.recent).map(installation=><Installation key={installation.installationRef} installation={installation} stations={stations} reload={reload}/>)}</details>}</>}</section>
  </div>;
}
function Installation({installation,stations,reload}:{installation:NonNullable<RuntimeProjection['unclaimed']>[number];stations:RuntimeProjection['stations'];reload:()=>void}) {
  return <article className={styles.installation} data-installation-ref={installation.installationRef}><div><h4>{installation.browser || '浏览器未知'} · 插件 {installation.pluginVersion}</h4><p>首次报到 {installation.firstSeen}</p></div>{stations && stations.length>0 ? <CommandForm action="runtime-claims" className={styles.form} fields={{installation_ref:installation.installationRef}} onWritten={reload}><select name="station_ref" aria-label="认领到哪台工位" required defaultValue=""><option value="">选择对应工位</option>{stations.map(s=><option key={s.stationRef} value={s.stationRef}>{s.name}</option>)}</select><ActionButton type="submit">认领到工位</ActionButton></CommandForm> : <span>{stations===null ? '工位列表读不到，暂不能认领' : '请先新增工位'}</span>}</article>;
}
