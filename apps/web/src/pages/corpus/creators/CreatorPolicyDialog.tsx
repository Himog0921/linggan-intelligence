import { useQuery } from '@tanstack/react-query';
import { useId, useRef, useState, type FormEvent } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { CreatorSelect } from './CreatorSelect';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { fetchModelChoices } from './creatorsApi';
import { feedbackForError, type Feedback } from './creatorsFeedback';
import type { CreatorPolicy } from './creatorsTypes';
import { FeedbackLine } from './FeedbackLine';
import { usePolicyMutation } from './useCreatorWrites';
import { useCreatorLeave } from './useCreatorLeave';
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

const LIMIT_MIN = 1024;
const LIMIT_MAX = 10_000_000;

export function CreatorPolicyDialog({ domain, policy, domainActive, onSaved, onClose }: CreatorPolicyDialogProps) {
  const titleId = useId();
  const begin = useWriteGuard(domain);
  const write = usePolicyMutation();
  const inFlight = useRef(false);
  const [threshold, setThreshold] = useState(policy.likeThreshold === null ? '' : String(policy.likeThreshold));
  const [enabled, setEnabled] = useState(policy.analysisEnabled);
  const [configRef, setConfigRef] = useState(policy.configRef ?? '');
  const [limit, setLimit] = useState(policy.dailyTokenLimit === null ? '' : String(policy.dailyTokenLimit));
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [busy, setBusy] = useState(false);
  const models = useQuery({ queryKey: ['model-choices'], queryFn: ({ signal }) => fetchModelChoices(signal), gcTime: 0 });
  // 模型配置读到了才能改分析设置；读不到时只存点赞标准，分析设置保持原值。
  const analysisEditable = models.isSuccess && domainActive;
  const choices = models.data ?? [];
  const knownConfig = choices.some(choice => choice.configRef === policy.configRef);

  const dirty = threshold !== (policy.likeThreshold === null ? '' : String(policy.likeThreshold)) || enabled !== policy.analysisEnabled || configRef !== (policy.configRef ?? '') || limit !== (policy.dailyTokenLimit === null ? '' : String(policy.dailyTokenLimit));
  const { requestClose, confirmation } = useCreatorLeave({ dirty, busy, onClose });

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (inFlight.current || !domainActive) return;
    if (!event.currentTarget.reportValidity()) return;
    const parsedLimit = limit.trim() === '' ? null : Number(limit);
    if (analysisEditable && enabled) {
      if (!configRef) return setFeedback({ tone: 'error', text: '启用分析前请选择模型配置。', code: null });
      if (parsedLimit === null || !Number.isSafeInteger(parsedLimit) || parsedLimit < LIMIT_MIN || parsedLimit > LIMIT_MAX) {
        return setFeedback({ tone: 'error', text: '启用分析前请填写有效的每日词元上限（1,024 到 10,000,000）。', code: null });
      }
    }
    const isCurrent = begin();
    inFlight.current = true;
    setBusy(true);
    setFeedback(null);
    try {
      await write.mutateAsync({
        domain,
        revision: policy.revision,
        likeThreshold: threshold.trim() === '' ? null : Number(threshold),
        ...(analysisEditable ? { analysis: { analysisEnabled: enabled, configRef: configRef || null, dailyTokenLimit: parsedLimit } } : {}),
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
    <>
    <ModalDialog labelledBy={titleId} className={styles.dialog} onClose={requestClose}>
      <form className={styles.form} onSubmit={event => void submit(event)}>
        <header className={styles.head}>
          <div>
            <small>仅影响当前领域</small>
            <h2 id={titleId}>发现设置</h2>
          </div>
          <button type="button" className={styles.textButton} onClick={requestClose}>
            关闭
          </button>
        </header>
        <div className={styles.body}>
          <section>
            <h3>爆款标准</h3>
            <label>
              最低点赞（留空表示不设标准）
              <input type="number" min={1} step={1} value={threshold} disabled={!domainActive} onChange={event => setThreshold(event.target.value)} />
            </label>
            <p className={styles.help}>仅重算已有材料的表现，不启动采集或模型。</p>
          </section>
          <section>
            <h3>自动分析</h3>
            <label className={styles.check}>
              <input type="checkbox" checked={enabled} disabled={!analysisEditable} onChange={event => setEnabled(event.target.checked)} />
              启用本领域作品与创作者来源分析
            </label>
            <p className={styles.help}>处理本领域已有材料及公开资料，使用已有模型与本用途额度；不采主页，不处理评论。</p>
            <div className={styles.selectField}><span>已有模型配置</span>
              <CreatorSelect label="已有模型配置" value={configRef} disabled={!analysisEditable} onChange={setConfigRef}
                options={[{ value: '', label: '选择模型配置' }, ...choices.map(choice => ({ value: choice.configRef, label: choice.label })),
                  ...(policy.configRef && !knownConfig ? [{ value: policy.configRef, label: '当前来源分析配置' }] : [])]} />
            </div>
            <label>
              本用途每日词元上限
              <input
                type="number"
                min={LIMIT_MIN}
                max={LIMIT_MAX}
                step={1}
                value={limit}
                disabled={!analysisEditable}
                onChange={event => setLimit(event.target.value)}
              />
            </label>
            <a href="/settings/models">模型设置</a>
            {models.isPending ? <p className={styles.help}>正在读取模型配置…</p> : null}
            {models.isError ? <p className={styles.help}>模型配置读取失败。可保存点赞标准，分析设置保持原值。</p> : null}
            {!domainActive ? <p className={styles.help}>当前领域已暂停，不能修改设置。</p> : null}
          </section>
        </div>
        <footer className={styles.foot}>
          <div className={styles.footNote}>
            <FeedbackLine feedback={feedback} />
          </div>
          <ActionButton onClick={requestClose}>取消</ActionButton>
          <ActionButton type="submit" variant="primary" busy={busy} disabled={!domainActive}>
            {busy ? '保存中…' : '保存设置'}
          </ActionButton>
        </footer>
      </form>
    </ModalDialog>
    {confirmation}
    </>
  );
}
