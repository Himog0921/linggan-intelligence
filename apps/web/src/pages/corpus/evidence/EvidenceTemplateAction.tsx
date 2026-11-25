import { useId, useState } from 'react';
import { Link } from 'react-router';
import { useQueryClient } from '@tanstack/react-query';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { ActionButton } from '../../../shared/ActionButton';
import { stepNames, type Step } from '../../workbench/workbenchModel';
import { createEvidenceTemplate, rejectedTemplateCommand, type EvidenceTemplateCommand } from '../../workbench/assets/structuralTemplatesApi';
import styles from '../../workbench/assets/Assets.module.css';

export function EvidenceTemplateAction({ workRef, domainRef, materialRevision }: {
  workRef: string;
  domainRef: string;
  materialRevision: string;
}) {
  const title = useId();
  const client = useQueryClient();
  const [open, setOpen] = useState(false);
  const [steps, setSteps] = useState<Step[]>([]);
  const [sourceRevision, setSourceRevision] = useState(materialRevision);
  const [pending, setPending] = useState<EvidenceTemplateCommand | null>(null);
  const [busy, setBusy] = useState(false);
  const [outcome, setOutcome] = useState<{ code: string } | 'duplicate' | null>(null);
  const [notice, setNotice] = useState('');
  const sourceChanged = sourceRevision !== materialRevision;
  const library = `/ops/creation?domain=${encodeURIComponent(domainRef)}&section=assets`;

  function openDialog() {
    if (!pending && sourceChanged) {
      setSteps([]);
      setSourceRevision(materialRevision);
      setNotice('');
    }
    setOpen(true);
  }

  async function save() {
    if (busy || outcome || (!pending && (!steps.length || sourceChanged))) return;
    const command = pending ?? { requestId: crypto.randomUUID(), workRef, domainRef, materialRevision: sourceRevision, steps };
    setPending(command);
    setBusy(true);
    setNotice('');
    try {
      const receipt = await createEvidenceTemplate(command);
      if (receipt.template.evidenceSource?.workRef !== command.workRef
        || receipt.template.evidenceSource.materialRevision !== command.materialRevision) throw new Error('invalid receipt');
      setPending(null);
      setOutcome({ code: receipt.template.code });
      await client.invalidateQueries({ queryKey: ['creation-assets', 'templates', domainRef] });
    } catch (error) {
      if (rejectedTemplateCommand(error) && error.failure.kind === 'http') {
        setPending(null);
        const code = error.failure.code;
        if (code === 'creation_assets_template_already_imported') {
          setOutcome('duplicate');
        } else {
          setNotice(code === 'creation_assets_source_unavailable'
            ? '研究底稿已变化或来源不可用，请刷新后重新选择结构。'
            : '无法保存结构，请核对所选步骤。');
        }
      } else {
        setNotice('保存结果尚未确认，请用原请求重试。');
      }
    } finally {
      setBusy(false);
    }
  }

  return <>
    <ActionButton variant="outline" onClick={openDialog}>复刻成样板</ActionButton>
    {open ? <ModalDialog labelledBy={title} className={styles.dialog} onClose={() => { if (!busy) setOpen(false); }}>
      <h2 id={title}>复刻成样板</h2>
      {outcome === 'duplicate' ? <p role="status">这篇作品已经入库。请在入库领域的样板库查看。</p>
        : outcome ? <>
          <p role="status">已保存 {outcome.code}，等待审核。</p>
          <Link to={library}>打开样板库</Link>
        </> : <>
        <p>根据这份研究底稿，按原作顺序选择写法步骤。候选只保存结构和来源版本。</p>
        <p>已选：{(pending?.steps ?? steps).map(step => stepNames[step]).join(' → ') || '尚未选择'}</p>
        <div className={styles.actions}>{Object.entries(stepNames).map(([key, label]) =>
          <ActionButton key={key} variant="quiet" disabled={busy || !!pending || sourceChanged || steps.length >= 12}
            onClick={() => setSteps(previous => [...previous, key as Step])}>＋ {label}</ActionButton>)}</div>
        <div className={styles.actions}>
          <ActionButton variant="quiet" disabled={busy || !!pending || sourceChanged || !steps.length}
            onClick={() => setSteps(previous => previous.slice(0, -1))}>撤销上一步</ActionButton>
          <ActionButton variant="primary" busy={busy} disabled={!pending && (!steps.length || sourceChanged)}
            onClick={() => void save()}>{pending ? '重试保存' : '保存待审核'}</ActionButton>
        </div>
        {notice || sourceChanged ? <p role="status">{notice || '底稿已更新，请关闭后重新提炼。'}</p> : null}
      </>}
      <div className={styles.actions}><ActionButton variant="quiet" disabled={busy} onClick={() => setOpen(false)}>关闭</ActionButton></div>
    </ModalDialog> : null}
  </>;
}
