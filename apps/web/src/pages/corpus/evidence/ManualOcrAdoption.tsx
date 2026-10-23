import { useRef, useState, type FormEvent } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { adoptManualOcr } from './ocrRecognitionApi';
import type { OcrRecognitionImage } from './ocrRecognitionTypes';
import styles from './OcrRecognition.module.css';

type Decision = 'adopt' | 'reject' | 'pending';
export function ManualOcrAdoption({ workRef, image, onSaved }: { workRef: string; image: OcrRecognitionImage; onSaved: () => void }) {
  const [open, setOpen] = useState(false); const [decision, setDecision] = useState<Decision>('adopt');
  const [busy, setBusy] = useState(false); const [message, setMessage] = useState(''); const retry = useRef<Parameters<typeof adoptManualOcr>[1] | null>(null);
  const canReview = Boolean(image.adoptionRef && image.originalUrl && image.state === 'succeeded');
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); if (!image.adoptionRef) return;
    const form = new FormData(event.currentTarget);
    const payload = retry.current ?? { commandRef: crypto.randomUUID(), adoptionRef: image.adoptionRef, expectedRevision: image.reviewRevision ?? 0,
      decision, field: 'adopted_body' as const, text: decision === 'adopt' ? String(form.get('text') ?? '') : null,
      actor: String(form.get('actor') ?? ''), reason: String(form.get('reason') ?? '') };
    setBusy(true); setMessage('');
    try { const receipt = await adoptManualOcr(workRef, payload); retry.current = null; setMessage(`人工采用已保存，第 ${receipt.revision} 版；正在重读当前结果。`); onSaved(); }
    catch (error) {
      retry.current = payload;
      setMessage(error instanceof ApiError && error.answered ? '当前版本或来源已变化，请重新读取后再决定。' : '保存回执暂未确认；请核对同一采用请求，不会创建第二版。');
    } finally { setBusy(false); }
  }
  if (!image.adoptionRef) return null;
  return <section className={styles.manual} aria-label="人工采用图片文字">
    <div className={styles.manualHeading}><b>{image.humanBody ? `已人工采用 · 第 ${image.humanBody.revision} 版` : '模型识别 · 未人工核验'}</b>
      {image.humanBody ? <small>{image.humanBody.actor} · {image.humanBody.reason}</small> : null}</div>
    {image.humanBody ? <pre>{image.humanBody.text}</pre> : null}
    {!canReview ? <p className={styles.status}>当前原图不可读或结果尚未保存，不能形成新的人工采用版本。</p> : <>
      <p className={styles.status}>请先对照原图。原始 OCR 与平台标题不会改写；无法对应保存区块的文字会标为人工转写。</p>
      <ActionButton onClick={() => setOpen(value => !value)}>{open ? '收起人工采用' : '人工核对并采用'}</ActionButton>
      {open ? <form className={styles.manualForm} onSubmit={event => void submit(event)}>
        <label>审核决定<select value={decision} disabled={busy || retry.current !== null} onChange={event => setDecision(event.target.value as Decision)}><option value="adopt">采用本图完整正文</option><option value="pending">保持待核验</option><option value="reject">拒绝当前机器预览</option></select></label>
        {decision === 'adopt' ? <label>对照原图后的完整正文<textarea name="text" required maxLength={16000} defaultValue={image.humanBody?.text ?? image.machineText ?? ''}/></label> : null}
        <label>审核人<input name="actor" required maxLength={120} autoComplete="off" /></label>
        <label>对照依据与原因<input name="reason" required maxLength={300} /></label>
        <ActionButton type="submit" busy={busy}>{retry.current ? '核对同一采用请求' : '保存人工采用'}</ActionButton>
      </form> : null}
    </>}
    {message ? <p className={styles.status} role="status">{message}</p> : null}
  </section>;
}
