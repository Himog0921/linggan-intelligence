import { useState } from 'react';
import { Link } from 'react-router';
import { decodeAssignment, type TargetList } from './api';
import { Modal } from './TargetModals';
import { TargetSelect } from './TargetSelect';
import { listHref, returnFields, useRead, pendingTargetFields, type Commands } from './useWorkspace';

export function AssignDomain({ targetRef, list, params, commands }: {
  targetRef: string; list: TargetList; params: URLSearchParams; commands: Commands;
}) {
  // Read the requested identity independently: the active list filter may exclude it.
  const { data, error } = useRead(`/api/collection/workspace/target/${encodeURIComponent(targetRef)}/assignment`, decodeAssignment);
  const domains = list.domains.filter((d) => d.status === 'active');
  const [domain, setDomain] = useState(() => { const value = pendingTargetFields('domain', { row_target_ref: targetRef })?.domain_ref; return typeof value === 'string' ? value : domains[0]?.domainRef ?? ''; });
  const close = listHref(params, { assign_domain: null }, `target-${targetRef}`);
  return (
    <Modal commands={commands} id="target-domain-assignment-title" className="c-tg-batch-dialog" closeHref={close}>
      <div>
        <h2 id="target-domain-assignment-title">{data ? `为「${data.name}」分配领域` : '读取领域分配对象'}</h2>
        <p>插件只负责带回博主身份，不替你选择研究领域。分配后先按 primary 角色关联，可在领域管理调整；作品材料仍进入共享材料链。</p>
      </div>
      {!data ? <p className="c-tg-delete-blocked">{error ?? '正在读取观察目标…'}</p>
        : !data.assignable ? <p className="c-tg-delete-blocked">这个目标已经有领域，没有覆盖现有领域。</p>
        : domains.length === 0 ? <p className="c-tg-delete-blocked">当前读不到任何可用领域，没有修改目标。领域读取恢复后再分配。</p>
        : <form onSubmit={(e) => {
          e.preventDefault();
          void commands.run('domain', { row_target_ref: targetRef, domain_ref: domain, ...returnFields(params) });
        }}>
          <label>领域<TargetSelect name="domain_ref" required label="领域" value={domain} onChange={setDomain}
            options={domains.map((d) => ({ value: d.domainRef, label: d.name }))} /></label>
          <div className="c-tg-batch-dialog-actions">
            <Link className="c-btn-secondary" to={close}>取消</Link>
            <button className="c-btn-primary" disabled={commands.busy}>确认分配</button>
          </div>
        </form>}
    </Modal>
  );
}
