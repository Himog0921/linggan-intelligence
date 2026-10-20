import { useId, useRef, useState, type FormEvent } from 'react';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { AiBusinessStatus } from '../../settings/AiBusinessStatus';
import { feedbackForError, type Feedback } from './creatorsFeedback';
import type { CreatorPolicy } from './creatorsTypes';
import { FeedbackLine } from './FeedbackLine';
import { usePolicyMutation } from './useCreatorWrites';
import { useWriteGuard } from './useWriteGuard';
import styles from './CreatorDialogs.module.css';

interface CreatorPolicyDialogProps {
  domain: string;
  /** 打开时读到的设置。保存带着它的修订号：设置已被别处更新就明确冲突，而不是悄悄覆盖。 */
  policy: CreatorPolicy;
  domainActive: boolean;
  /** 保存已被服务器确认，且发起它的这个对话框还开着时才调用。 */
  onSaved: () => void;
  onClose: () => void;
}

export function CreatorPolicyDialog({ domain, policy, domainActive, onSaved, onClose }: CreatorPolicyDialogProps) {
  const titleId = useId();
  const begin = useWriteGuard(domain);
  const write = usePolicyMutation();
  const inFlight = useRef(false);
  const [threshold, setThreshold] = useState(policy.likeThreshold === null ? '' : String(policy.likeThreshold));
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (inFlight.current || !domainActive) return;
    if (!event.currentTarget.reportValidity()) return;
    const isCurrent = begin();
    inFlight.current = true;
    setBusy(true);
    setFeedback(null);
    try {
      await write.mutateAsync({
        domain,
        revision: policy.revision,
        likeThreshold: threshold.trim() === '' ? null : Number(threshold),
      });
      if (isCurrent()) onSaved();
    } catch (error) {
      // 重复保存同一修订号：第一次若已生效，第二次会得到明确的版本冲突，而不是悄悄覆盖。
      if (isCurrent()) setFeedback(feedbackForError(error, { retryNote: '重新保存会带着同一个设置版本，若刚才已生效会提示设置已更新，不会重复生效。' }));
    } finally {
      inFlight.current = false;
      if (isCurrent()) setBusy(false);
    }
  }

  return (
    <ModalDialog labelledBy={titleId} className={styles.dialog} onClose={onClose}>
      <form className={styles.form} onSubmit={event => void submit(event)}>
        <header className={styles.head}>
          <div>
            <small>仅影响当前领域</small>
            <h2 id={titleId}>发现设置</h2>
          </div>
          <button type="button" className={styles.textButton} onClick={onClose}>
            关闭
          </button>
        </header>
        <div className={styles.body}>
          <section>
            <h3>爆款标准</h3>
            {!domainActive ? <p className={styles.help}>当前领域已暂停，不能修改设置。</p> : null}
            <label>
              最低点赞（留空表示不设标准）
              <input type="number" min={1} step={1} value={threshold} disabled={!domainActive} onChange={event => setThreshold(event.target.value)} />
            </label>
            <p className={styles.help}>仅重算已有材料的表现，不启动采集或模型。</p>
          </section>
          <AiBusinessStatus taskKey="legacy_creator_discovery" />
        </div>
        <footer className={styles.foot}>
          <div className={styles.footNote}>
            <FeedbackLine feedback={feedback} />
          </div>
          <button type="button" onClick={onClose}>
            取消
          </button>
          <button type="submit" className={styles.primary} disabled={busy || !domainActive}>
            {busy ? '保存中…' : '保存设置'}
          </button>
        </footer>
      </form>
    </ModalDialog>
  );
}
