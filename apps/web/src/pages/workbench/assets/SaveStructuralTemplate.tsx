import { useId, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { ActionButton } from '../../../shared/ActionButton';
import type { Revision } from '../workbenchModel';
import * as api from './structuralTemplatesApi';
import { assetsErrorText } from './assetsApi';
import styles from './Assets.module.css';

export function SaveStructuralTemplate({ workId, domainRef, saveCurrent, disabled, pending, onPending, blocked, onResetBlock }: {
  workId: string; domainRef: string; saveCurrent: () => Promise<Revision>; disabled: boolean;
  pending: api.CreateCommand | null; onPending: (command: api.CreateCommand | null) => void; blocked: boolean; onResetBlock: () => void;
}) {
  const uid = useId(); const client = useQueryClient();
  const [open, setOpen] = useState(false); const [steps, setSteps] = useState<api.Step[]>([]);
  const [busy, setBusy] = useState(false); const [notice, setNotice] = useState('');
  async function save() {
    if (busy || (!pending && !steps.length)) return;
    setBusy(true); setNotice('');
    let command = pending;
    try {
      if (!command) {
        const revision = await saveCurrent();
        command = { requestId: crypto.randomUUID(), workId, expectedRevision: revision.id, steps };
        onPending(command);
      }
      const result = await api.createTemplate(command);
      if (result.template.sourceWorkId !== command.workId || result.template.sourceRevisionId !== command.expectedRevision) throw new Error('invalid receipt');
      onPending(null); setSteps([]); setNotice(`已保存 ${result.template.code}，等待审核。`);
      await client.invalidateQueries({ queryKey: ['creation-assets', 'templates', domainRef] });
    } catch (error) {
      if (!command) setNotice('稿件保存未完成，请重试。');
      else if (api.rejectedTemplateCommand(error)) { onPending(null); setNotice(assetsErrorText(error)); }
      else setNotice('保存结果尚未确认，请沿用原请求重试。');
    } finally { setBusy(false); }
  }
  const blockedNotice = blocked ? <p role="alert">样板保存结果未确认，请先留在此稿并重试。 <button type="button" onClick={onResetBlock}>返回稿件</button></p> : null;
  return <>{!open ? blockedNotice : null}<ActionButton variant="outline" disabled={disabled && !pending} onClick={() => { setNotice(''); setOpen(true); }}>存为样板</ActionButton>{open ? <ModalDialog labelledBy={`${uid}-title`} className={styles.dialog} onClose={() => { if (!busy) setOpen(false); }}>
    <h2 id={`${uid}-title`}>存为样板</h2>{blockedNotice}<p>按顺序选择写法结构。只保存结构和当前稿件版本的关联，不复制正文；保存后需在资产库审核。</p>
    <p>已选：{(pending?.steps ?? steps).map(step => api.stepNames[step]).join(' → ') || '尚未选择'}</p>
    <div className={styles.actions}>{Object.entries(api.stepNames).map(([key, name]) => <ActionButton key={key} variant="quiet" disabled={busy || !!pending || steps.length >= 12} onClick={() => setSteps(value => [...value, key as api.Step])}>＋ {name}</ActionButton>)}</div>
    <div className={styles.actions}><ActionButton variant="quiet" disabled={busy || !!pending || !steps.length} onClick={() => setSteps(value => value.slice(0, -1))}>撤销上一步</ActionButton></div>
    {notice ? <p role="status">{notice}</p> : null}<div className={styles.actions}><ActionButton variant="outline" disabled={busy} onClick={() => setOpen(false)}>关闭</ActionButton><ActionButton variant="primary" busy={busy} disabled={!pending && !steps.length} onClick={() => void save()}>{pending ? '重试保存' : '保存待审核'}</ActionButton></div>
  </ModalDialog> : null}</>;
}
