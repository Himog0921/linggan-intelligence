import { useEffect, useRef, useState } from 'react';
import { useBlocker } from 'react-router';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { ActionButton } from '../../../shared/ActionButton';
import { loadRuntime } from './managementApi';
import { RuntimeContent } from './RuntimeContent';
import { useManagementRead } from './useManagementRead';
import styles from './RuntimeManagementDialog.module.css';

function hasDraft(container: HTMLElement) {
  return [...container.querySelectorAll('form input,form select,form textarea')].some(field => {
    if (field instanceof HTMLInputElement) return field.type === 'checkbox' || field.type === 'radio' ? field.checked !== field.defaultChecked : field.type !== 'hidden' && field.value !== field.defaultValue;
    if (field instanceof HTMLSelectElement) { const initial = [...field.options].find(option=>option.defaultSelected) ?? field.options[0]; return field.value !== initial?.value; }
    return field instanceof HTMLTextAreaElement && field.value !== field.defaultValue;
  });
}
export function RuntimeManagementDialog({onClose}:{onClose:()=>void}) {
  const read = useManagementRead(loadRuntime);
  const content = useRef<HTMLDivElement>(null);
  const [leaving,setLeaving] = useState<'draft'|'pending'|null>(null);
  const allowingExit=useRef(false);
  function leaveReason() {
    const body=content.current;
    return body?.querySelector('form[aria-busy="true"],form[data-command-unconfirmed="true"]') ? 'pending' : body && hasDraft(body) ? 'draft' : null;
  }
  const blocker=useBlocker(()=>{if(allowingExit.current)return false;const reason=leaveReason();if(reason)setLeaving(reason);return reason!==null;});
  useEffect(()=>{const warn=(event:BeforeUnloadEvent)=>{if(leaveReason()){event.preventDefault();event.returnValue='';}};window.addEventListener('beforeunload',warn);return()=>window.removeEventListener('beforeunload',warn);},[]);
  function requestClose() {const reason=leaveReason();if(reason)setLeaving(reason);else onClose();}
  function keepEditing(){setLeaving(null);if(blocker.state==='blocked')blocker.reset();}
  function completeLeave(){allowingExit.current=true;if(blocker.state==='blocked')blocker.proceed();else onClose();}
  return <ModalDialog className={`c-run-dialog ${styles.dialog}`} labelledBy="runtime-manager-title" closeOnBackdrop onClose={requestClose} setup={dialog=>{document.documentElement.classList.add('c-run-dialog-open');return()=>{if(![...document.querySelectorAll('dialog[open]')].some(open=>open!==dialog))document.documentElement.classList.remove('c-run-dialog-open');};}}>
    <header className={styles.heading}><h2 id="runtime-manager-title">工位管理</h2><ActionButton variant="quiet" data-initial-focus aria-label="关闭工位管理" onClick={requestClose}>关闭</ActionButton></header>
    <div ref={content} className={styles.body}>
      {read.error && <p role="alert">工位信息读取失败。{read.data ? `以下保留上次读取结果（${read.data.asOf ?? '时间未知'}）。` : '请重新读取。'}<ActionButton onClick={()=>void read.reload()}>重新读取</ActionButton></p>}
      {read.data?.projection ? <RuntimeContent data={read.data.projection} reload={()=>void read.reload()}/> : read.loading ? <p role="status">正在读取工位…</p> : <p>工位配置当前读不到，未知状态没有显示为零。</p>}
    </div>
    {leaving && <section className={styles.leave} role="alert"><p>{leaving==='pending' ? '操作结果尚未确认。收起后会保留原请求，再次打开时先核实同一请求。' : '有尚未保存的输入，关闭将放弃这些修改。'}</p><div><ActionButton autoFocus onClick={keepEditing}>继续处理</ActionButton><ActionButton variant={leaving==='pending'?'outline':'danger'} onClick={completeLeave}>{leaving==='pending'?'收起并保留请求':'放弃修改并关闭'}</ActionButton></div></section>}
  </ModalDialog>;
}
