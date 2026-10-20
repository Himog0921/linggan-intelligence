import { ActionButton } from '../../../shared/ActionButton';
import { CommandForm } from './CommandForm';
import type { StationEntry } from './managementApi';
import styles from './Management.module.css';

function capabilityLabel(capability: string, label: string) {
  const labels: Record<string,string> = {'collection.single_run.v1':'单次采集','collection.all_public.v1':'深度评论采集',batch_checkpoint:'批次进度记录'};
  return labels[capability] ?? (label === capability && !/[\u3400-\u9fff]/.test(label) ? '其他能力' : label);
}

export function StationSettings({ station, accountAvailable, reload }: { station: StationEntry; accountAvailable: boolean; reload: () => void }) {
  const account = station.account;
  return <div className={styles.settings}>
    <section className={styles.capabilities}><h4>工位能力 <small>近 7 天</small></h4>{station.capabilities === null ? <p>能力矩阵当前读不到，不能据此认定没有能力。</p> : station.capabilities.length === 0 ? <p>尚无能力记录。</p> : station.capabilities.map(cap => <div className={styles.capability} key={cap.capability}><b>{capabilityLabel(cap.capability,cap.label)}</b><span className={`c-tg-truth ${cap.className}`}>{cap.stateWord}</span><p>{cap.detail}{cap.executionFailures > 0 && <> · 另有 {cap.executionFailures} 次执行失败</>}</p></div>)}</section>
    <section><h4>连接插件</h4><p>{station.claimWindowOpen ? '认领窗口已开启。新安装的插件会自动归入这台工位。' : '开启认领窗口后，新安装的插件会自动归入这台工位。已报到的安装在“插件认领”中处理。'}</p><CommandForm action={station.claimWindowOpen ? 'runtime-close-window' : 'runtime-claim-window'} className={styles.form} fields={{ station_ref: station.stationRef }} onWritten={reload}>{station.claimWindowOpen ? <ActionButton type="submit">关闭认领窗口</ActionButton> : <><label htmlFor={`claim-hours-${station.stationRef}`}>有效期</label><select id={`claim-hours-${station.stationRef}`} name="valid_for_hours" defaultValue="24"><option value="24">24 小时</option><option value="8">8 小时</option><option value="1">1 小时</option></select><ActionButton type="submit">开启认领窗口</ActionButton></>}</CommandForm></section>
    <section><h4>接活与账号</h4>{account ? <><dl className={styles.facts}><div><dt>插件安装</dt><dd>{account.installationLabel}</dd></div><div><dt>连接凭据</dt><dd>{account.credentialValid ? '有效' : '缺失或已失效'}</dd></div><div><dt>账号绑定</dt><dd>{account.bindingLabel}</dd></div><div><dt>最近观察</dt><dd>{account.observed ?? '尚未观察'}</dd></div></dl><p>{account.hasLiveLease ? '账号当前有任务在执行。' : '账号当前没有执行中的任务。'}{!accountAvailable && '本机暂未配置账号身份摘要，登录与访问限制仍会影响接活。'}</p><div className={styles.form}><CommandForm action="runtime-accepting" fields={{ station_ref: station.stationRef, accepting: !account.accepting }} onWritten={reload}><ActionButton type="submit">{account.accepting ? '暂停接活' : '恢复自动接活'}</ActionButton></CommandForm>{account.installationRef && account.accountRef && ['unconfirmed', 'changed'].includes(account.bindingState) && <CommandForm action="runtime-account-bindings" fields={{ installation_ref: account.installationRef, account_ref: account.accountRef }} onWritten={reload}><ActionButton type="submit">确认这个观察账号</ActionButton></CommandForm>}</div></> : <p>接活开关与账号绑定当前读不到，不能据此认定没有绑定。</p>}</section>
    <section><h4>工位设置</h4><CommandForm action="runtime-rename" className={styles.form} fields={{ station_ref: station.stationRef }} onWritten={reload}><label htmlFor={`station-name-${station.stationRef}`}>名称</label><input id={`station-name-${station.stationRef}`} name="display_name" required maxLength={60} defaultValue={station.name}/><ActionButton type="submit">保存名称</ActionButton></CommandForm><p>停用后不再自动领取新任务。</p><CommandForm action="runtime-retire" fields={{ station_ref: station.stationRef }} onWritten={reload}><ActionButton variant="danger" type="submit">停用这台工位</ActionButton></CommandForm></section>
  </div>;
}

export function RegisterStation({ reload, onApplied }: { reload: () => void; onApplied: () => void }) {
  return <section className={styles.register} aria-labelledby="station-register-title"><h3 id="station-register-title">新增工位</h3><p>先为执行设备命名，再在这台工位中开启认领窗口，连接浏览器插件。</p><CommandForm action="runtime-register" className={styles.form} onWritten={reload} onApplied={onApplied} resetOnApplied><label htmlFor="station-name">工位名称</label><input id="station-name" name="display_name" required maxLength={60} defaultValue="本机 Chrome"/><ActionButton variant="primary" type="submit">登记工位</ActionButton></CommandForm><p>登记只保存本地工位信息，不会启动采集。</p></section>;
}
