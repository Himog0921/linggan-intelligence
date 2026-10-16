import { useEffect, useRef, useState, type ReactNode } from 'react';

interface ModalDialogProps {
  /** 标题元素的 id，作为对话框的可访问名称。 */
  labelledBy: string;
  className?: string;
  /** Esc 或其他原生取消动作。由调用方决定是否真的关闭；默认直接关闭。 */
  onClose: () => void;
  /** 关闭后默认回到打开前的焦点；打开者已不在页面上时用它指定落点。 */
  restoreFocus?: () => HTMLElement | null;
  /** 打开之后需要对 `<dialog>` 做的额外设置（例如按页头高度定位抽屉）；返回清理函数。 */
  setup?: (dialog: HTMLDialogElement) => void | (() => void);
  children: ReactNode;
}

/**
 * 原生 `<dialog>` 的模态包装：挂载即 `showModal()`（浏览器负责焦点陷阱与背景惰性），卸载即关闭。
 * 每次打开都是一个新的实例（调用方用 key 区分对象），它的状态和进行中的请求都不会串到下一次打开。
 */
export function ModalDialog({ labelledBy, className, onClose, restoreFocus, setup, children }: ModalDialogProps) {
  const ref = useRef<HTMLDialogElement>(null);
  // 打开前的焦点位置：关闭后还给它，键盘用户不会被丢回页面顶部。
  const [opener] = useState(() => (document.activeElement instanceof HTMLElement ? document.activeElement : null));
  const [fallback] = useState(() => restoreFocus);
  const [configure] = useState(() => setup);

  useEffect(() => {
    const dialog = ref.current;
    if (dialog && !dialog.open) dialog.showModal();
    const cleanup = dialog ? configure?.(dialog) : undefined;
    return () => {
      cleanup?.();
      if (dialog?.open) dialog.close();
      const target = opener?.isConnected ? opener : (fallback?.() ?? null);
      target?.focus({ preventScroll: true });
    };
  }, [opener, fallback, configure]);

  return (
    <dialog
      ref={ref}
      className={className}
      aria-labelledby={labelledBy}
      onCancel={event => {
        event.preventDefault();
        onClose();
      }}
    >
      {children}
    </dialog>
  );
}
