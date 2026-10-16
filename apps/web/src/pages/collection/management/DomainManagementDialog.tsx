import { useEffect, useRef, useState } from 'react';
import { CommandForm } from './CommandForm';
import { DomainManagementContent } from './DomainManagementContent';
import { loadDomains } from './managementApi';
import { useManagementRead } from './useManagementRead';

export function DomainManagementDialog({ domainRef, onClose }: { domainRef?: string; onClose: () => void }) {
  const read = useManagementRead(loadDomains);
  const [editing, setEditing] = useState<string | null>(domainRef ?? null);
  const dialog = useRef<HTMLDialogElement>(null);
  const body = useRef<HTMLDivElement>(null);
  const [returnFocus] = useState(() => document.activeElement instanceof HTMLElement ? document.activeElement : null);
  useEffect(() => { const element = dialog.current; element?.showModal(); document.documentElement.classList.add("c-run-dialog-open"); return () => { element?.close(); document.documentElement.classList.remove("c-run-dialog-open"); returnFocus?.focus(); }; }, [returnFocus]);
  useEffect(() => { if (editing && read.data) { if (body.current) body.current.scrollTop = 0; body.current?.querySelector<HTMLInputElement>('input[name="name"]')?.focus({ preventScroll: true }); } }, [editing, read.data]);
  const current = new URLSearchParams(window.location.search).get('domain');
  function enterDomain(domain: string) {
    const destination = new URL(window.location.href); destination.searchParams.set('domain', domain); destination.searchParams.delete('page');
    window.location.assign(destination.href);
  }
  return <dialog ref={dialog} className="c-run-dialog c-domain-manager" aria-labelledby="domain-manager-title" onCancel={event => { event.preventDefault(); onClose(); }}>
    <header><div><h2 id="domain-manager-title">领域管理</h2><p>整理关注范围，在同一个工作台切换采集上下文。</p></div><button type="button" className="c-btn-quiet" data-close-domain aria-label="关闭领域管理" onClick={onClose}>×</button></header>
    <div className="c-run-dialog-body" data-domain-content ref={body}>
      {read.error && <p role="alert">领域配置当前读不到；{read.data ? "以下保留上次读取结果与草稿。" : "这不表示没有领域。"}<button type="button" onClick={() => void read.reload()}>重新读取</button></p>}{!read.data ? read.loading ? <p role="status">正在读取领域…</p> : null : !read.data.readable ? <p>领域配置和关联关系当前读不到；页面未把未知状态显示为零。</p> : editing ? <><button className="c-btn-quiet" type="button" onClick={() => setEditing(null)}>← 返回领域列表</button><DomainManagementContent read={read.data} reload={() => { void read.reload(); }} onlyDomain={editing} onSaved={() => setEditing(null)} /></> : <>
        <div className="c-run-toolbar"><b>{read.data.domains.length} 个领域</b><button className="c-btn-secondary" type="button" onClick={() => setEditing('new')}>＋ 新建领域</button></div>
        {read.data.domains.map(domain => <article className="c-domain-list-row" key={domain.domainRef}><div><b>{domain.name}</b>{domain.domainRef === current && <> <small className="c-domain-current">当前</small></>}<p>{domain.description || '暂无领域说明'}</p></div><span>{domain.targetCount} 目标</span><span className="c-domain-status" data-active={domain.status === 'active'}>{domain.status === 'active' ? '运行中' : '已暂停'}</span><div className="c-run-controls"><button type="button" className="c-btn-quiet" onClick={() => enterDomain(domain.domainRef)}>进入</button><button type="button" className="c-btn-quiet" onClick={() => setEditing(domain.domainRef)}>编辑</button><CommandForm action="domain-save" onWritten={() => void read.reload()} fields={{ domain_ref: domain.domainRef, name: domain.name, description: domain.description ?? '', research_goal: domain.researchGoal ?? '', status: domain.status === 'active' ? 'paused' : 'active' }}><button type="submit" className="c-btn-quiet">{domain.status === 'active' ? '暂停' : '恢复'}</button></CommandForm></div></article>)}
        <p className="c-domain-note">暂停领域会暂缓该领域的新采集和排队任务，历史材料仍可查看；已取得的数据继续交付。目标可被多个领域关联，详情按来源复用。</p>
      </>}
    </div>
    <footer><button className="c-btn-secondary" type="button" onClick={onClose}>完成</button></footer>
  </dialog>;
}
