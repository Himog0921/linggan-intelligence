import { ActionFeedback, type ReceiptState } from '../../../shared/ActionFeedback';
import type { Feedback } from './creatorsFeedback';

const STATE: Record<Feedback['tone'], ReceiptState> = {
  success: 'applied',
  error: 'rejected',
  unknown: 'unconfirmed',
};

/** 动作回执走 LIDS C44：阶段先用文字说清，颜色只做加强。 */
export function FeedbackLine({ feedback, id }: { feedback: Feedback | null; id?: string }) {
  return (
    <ActionFeedback id={id} state={feedback ? STATE[feedback.tone] : undefined} code={feedback?.code}>
      {feedback?.text}
    </ActionFeedback>
  );
}
