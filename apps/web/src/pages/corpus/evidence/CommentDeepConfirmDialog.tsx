import { ActionButton } from '../../../shared/ActionButton';
import { useId } from 'react';
import { ModalDialog } from '../../../corpus/ModalDialog';
import styles from './Dialogs.module.css';

interface CommentDeepConfirmDialogProps {
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * 评论深采的确认（Mog 2026-10-23）。和「立即复观测」一样：会让工位真实访问平台，所以先说清会做什么、
 * 不会做什么，确认之后才发请求；「取消」排在「确认提交」之前，是第一个可聚焦的控件。
 * 真实动作走采集页同一条单次采集合同（只含这一篇、尽量采全），不另造一套。
 */
export function CommentDeepConfirmDialog({ onConfirm, onCancel }: CommentDeepConfirmDialogProps) {
  const titleId = useId();
  return (
    <ModalDialog labelledBy={titleId} className={styles.dialog} onClose={onCancel}>
      <div className={styles.form}>
        <header className={styles.head}>
          <div>
            <small>作品 · 评论深采</small>
            <h2 id={titleId}>确认评论深采</h2>
          </div>
        </header>
        <div className={styles.body}>
          <p>这会让合格的工位真实访问平台，打开这篇笔记，把能看到的评论和回复尽量采全。提交后可以在采集任务里暂停或停止，已经采到的不会删除。</p>
          <p>这一步会：</p>
          <ul className={styles.list}>
            <li>新建一次只包含这篇笔记的单次采集</li>
            <li>从评论区开头重新采，和以前采到的重复的评论不会重复计数</li>
            <li>如果这篇还没采过详情，会先补采详情和图片视频</li>
          </ul>
          <p>这一步不会：</p>
          <ul className={styles.list}>
            <li>调用模型</li>
            <li>立即得到结果——要等工位执行；评论多的笔记会分几段采</li>
          </ul>
        </div>
        <footer className={styles.foot}>
          <span className={styles.spacer} />
          <button type="button" onClick={onCancel}>
            取消
          </button>
          <ActionButton variant="primary" onClick={onConfirm}>
            确认提交
          </ActionButton>
        </footer>
      </div>
    </ModalDialog>
  );
}
