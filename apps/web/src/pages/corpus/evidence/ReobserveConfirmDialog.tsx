import { useId } from 'react';
import { ModalDialog } from '../../../corpus/ModalDialog';
import styles from './Dialogs.module.css';

interface ReobserveConfirmDialogProps {
  /** 上一次提交没有得到答复：可能已经入队，再提交可能再形成一张工单。 */
  afterUnknown: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * 立即复观测的确认（DR-3）。「取消」排在「确认提交」之前，是对话框里第一个可聚焦的控件——原生模态对话框打开时
 * 焦点落在它上：真实世界的动作不靠一次 Enter 误触发。这一步会让合格的工位真实访问平台，且提交后不能撤销，所以和「加入观察」一样先说清后果、
 * 确认之后才发请求。接口没有请求编号：同一范围若已有尚未被认领的工单则合并；一旦被认领，再提交会新建一张，
 * 工位会再访问一次平台——未确认之后再次提交时把这件事写明。
 */
export function ReobserveConfirmDialog({ afterUnknown, onConfirm, onCancel }: ReobserveConfirmDialogProps) {
  const titleId = useId();
  return (
    <ModalDialog labelledBy={titleId} className={styles.dialog} onClose={onCancel}>
      <div className={styles.form}>
        <header className={styles.head}>
          <div>
            <small>作品 · 观察记录</small>
            <h2 id={titleId}>确认立即复观测</h2>
          </div>
        </header>
        <div className={styles.body}>
          <p>这会让合格的工位真实访问平台，重新读取这篇作品的详情、评论与回复。提交后不能撤销。</p>
          {afterUnknown ? (
            <p className={styles.notice}>
              上一次请求没有得到答复，可能已经入队。如果那张工单已被工位认领，再提交会新建一张工单，工位会再访问一次平台。建议先点“重新核对”确认。
            </p>
          ) : null}
          <p>这一步会：</p>
          <ul className={styles.list}>
            <li>在该作品原先关联的有效授权范围内，创建一张立即复观测的工单</li>
            <li>读取详情、评论（最多 30 条）与回复</li>
          </ul>
          <p>这一步不会：</p>
          <ul className={styles.list}>
            <li>下载新的媒体，或运行图片文字、转录</li>
            <li>调用模型</li>
            <li>立即得到结果——工单要等工位认领后才会执行</li>
          </ul>
        </div>
        <footer className={styles.foot}>
          <span className={styles.spacer} />
          <button type="button" onClick={onCancel}>
            取消
          </button>
          <button type="button" className={styles.primary} onClick={onConfirm}>
            {afterUnknown ? '仍然提交' : '确认提交'}
          </button>
        </footer>
      </div>
    </ModalDialog>
  );
}
