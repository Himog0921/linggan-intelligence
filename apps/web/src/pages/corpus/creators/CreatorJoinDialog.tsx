import { useId, useRef, useState } from 'react';
import { ModalDialog } from '../../../corpus/ModalDialog';
import type { RegisterReceipt } from './creatorsTypes';
import { feedbackForError, type Feedback } from './creatorsFeedback';
import { FeedbackLine } from './FeedbackLine';
import { useJoinMutation } from './useCreatorWrites';
import { useWriteGuard } from './useWriteGuard';
import styles from './CreatorDialogs.module.css';

/** 要发出的「加入观察」请求，以及对话框为了说清后果需要知道的事实。 */
export interface JoinTarget {
  domain: string;
  domainName: string;
  creatorKey: string;
  authorName: string;
  /** 实际发送的用途；其他领域已有主研究关系时，主研究会被改为参照。 */
  role: 'primary' | 'reference';
  blockedPrimary: boolean;
  /** 该作者已有的观察目标当前处于暂停/忽略时的说法；加入不会恢复它。 */
  keptState: string | null;
}

interface CreatorJoinDialogProps {
  target: JoinTarget;
  onClose: () => void;
}

type Phase = 'confirm' | 'sending' | 'failed' | 'done';

function successText(target: JoinTarget, receipt: RegisterReceipt): string {
  if (!receipt.relationshipAdded) return `「${target.authorName}」已在「${target.domainName}」的观察名单中，本次没有新增变化。`;
  const where = receipt.usageRole === 'reference' ? '的参照用途' : '的观察名单';
  const registered = receipt.created ? '已新登记这位作者。' : '沿用了已有的观察目标。';
  const kept = target.keptState ? `目标保持「${target.keptState}」。` : '';
  const monitoring =
    receipt.monitoringEnabled === false ? '巡查尚未开启。' : receipt.monitoringEnabled === true ? '巡查沿用该目标原有设置。' : '';
  return `已把「${target.authorName}」加入「${target.domainName}」${where}。${registered}${kept}${monitoring}`;
}

/**
 * 「加入观察」的确认。先说清会发生什么、不会发生什么，确认之后才发出请求；回执之后才显示结果。
 *
 * 接口没有请求编号，但按（作者键，领域）天然幂等：重复提交得到同一个观察目标和同一个关系。所以
 * 「未确认」之后的重试发送的就是同一个请求体，不会产生第二个目标。结果只显示在这个对话框里：
 * 对话框关掉、或之后为别的作者开了新的对话框，迟到的回执只会触发列表刷新，不会改动任何界面。
 */
export function CreatorJoinDialog({ target, onClose }: CreatorJoinDialogProps) {
  const titleId = useId();
  const begin = useWriteGuard(`${target.domain}:${target.creatorKey}:${target.role}`);
  const write = useJoinMutation();
  const inFlight = useRef(false);
  const [phase, setPhase] = useState<Phase>('confirm');
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const roleText = target.role === 'reference' ? '参照' : '主研究';

  async function confirm() {
    if (inFlight.current) return;
    const isCurrent = begin();
    inFlight.current = true;
    setPhase('sending');
    setFeedback(null);
    try {
      const receipt = await write.mutateAsync({ domain: target.domain, creatorKey: target.creatorKey, usageRole: target.role });
      if (isCurrent()) {
        setFeedback({ tone: 'success', text: successText(target, receipt), code: null });
        setPhase('done');
      }
    } catch (error) {
      if (isCurrent()) {
        setFeedback(feedbackForError(error, { retryNote: '重试发送的是同一个请求：同一位作者在同一个领域只会有一个观察目标，不会重复登记。' }));
        setPhase('failed');
      }
    } finally {
      inFlight.current = false;
    }
  }

  return (
    <ModalDialog labelledBy={titleId} className={styles.dialog} onClose={onClose}>
      <div className={styles.form}>
        <header className={styles.head}>
          <div>
            <small>创作者 · 观察名单</small>
            <h2 id={titleId}>加入观察目标</h2>
          </div>
          <button type="button" className={styles.textButton} aria-label="关闭加入观察对话框" onClick={onClose}>
            关闭
          </button>
        </header>
        <div className={styles.body}>
          <p>
            把「{target.authorName}」加入「{target.domainName}」的观察名单，用途：<b>{roleText}</b>。
          </p>
          {target.blockedPrimary ? (
            <p className={styles.notice}>其他领域已有主研究关系；本领域只能按参照用途加入，不改变原巡查规则。</p>
          ) : null}
          {target.keptState ? (
            <p className={styles.notice}>该作者已有观察目标，当前为「{target.keptState}」；加入后保持这个状态，不会自动恢复。</p>
          ) : null}
          <p>这一步会：</p>
          <ul className={styles.list}>
            <li>登记这位作者为本机观察目标（已登记过则沿用，不重复建立）</li>
            <li>建立这位作者与本领域的关系（用途：{roleText}）</li>
          </ul>
          <p>这一步不会：</p>
          <ul className={styles.list}>
            <li>立即采集作品、评论或主页</li>
            <li>开启或关闭巡查，也不改变已有的巡查规则</li>
            <li>调用模型</li>
          </ul>
          <FeedbackLine feedback={feedback} />
          {phase === 'sending' ? (
            <p role="status" className={styles.help}>
              正在加入…
            </p>
          ) : null}
        </div>
        <footer className={styles.foot}>
          <span className={styles.spacer} />
          {phase === 'done' ? (
            <button type="button" className={styles.primary} autoFocus onClick={onClose}>
              关闭
            </button>
          ) : (
            <>
              <button type="button" onClick={onClose}>
                {phase === 'failed' ? '关闭' : '取消'}
              </button>
              <button type="button" className={styles.primary} disabled={phase === 'sending'} onClick={() => void confirm()}>
                {phase === 'sending' ? '加入中…' : phase === 'failed' ? (feedback?.tone === 'unknown' ? '重试（同一请求）' : '重新提交') : '确认加入'}
              </button>
            </>
          )}
        </footer>
      </div>
    </ModalDialog>
  );
}
