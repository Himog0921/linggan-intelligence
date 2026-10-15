import type { Feedback } from './modelSettingsModel';
import styles from './ModelSettingsPage.module.css';

const TONE_LABEL: Record<Feedback['tone'], string> = {
  success: '已完成',
  error: '未完成',
  unknown: '未确认',
};

/** 动作回执。状态同时用文字、位置和颜色表达，不只靠颜色（LIDS）。 */
export function FeedbackLine({ feedback, id }: { feedback: Feedback | null; id?: string }) {
  return (
    <p id={id} role="status" aria-live="polite" className={feedback ? `${styles.feedback} ${styles[feedback.tone]}` : styles.feedbackEmpty}>
      {feedback ? (
        <>
          <b className={styles.feedbackTone}>{TONE_LABEL[feedback.tone]}</b>
          <span>{feedback.text}</span>
          {feedback.code ? <code className={styles.code}>{feedback.code}</code> : null}
        </>
      ) : null}
    </p>
  );
}
