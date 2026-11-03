import { createContext, useContext } from 'react';
import type { FeedbackTone } from './evidenceFeedback';

/** 一次操作结果的最短写法；只承载回执，不替代就地状态、错误或未确认入口。 */
export interface ToastNotice {
  /** 同一作品同一动作的重复回执替换上一条，而不是叠着出现。 */
  key: string;
  tone: FeedbackTone;
  text: string;
  /** 作品或底稿的短名，避免 A 的迟到结果看起来像 B 的。 */
  context?: string;
}

export interface ToastApi { notify: (notice: ToastNotice) => void }

const noop = () => undefined;
const NO_TOAST: ToastApi = { notify: noop };
export const ToastContext = createContext<ToastApi | null>(null);

/** 没有局部通知层的测试或独立组件里，回执只留在就地状态中。 */
export function useToast(): ToastApi {
  return useContext(ToastContext) ?? NO_TOAST;
}
