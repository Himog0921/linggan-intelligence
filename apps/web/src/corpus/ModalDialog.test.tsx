import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ModalDialog } from './ModalDialog';

function setup(closeOnBackdrop?: boolean) {
  const onClose = vi.fn();
  render(<ModalDialog labelledBy="heading" onClose={onClose} closeOnBackdrop={closeOnBackdrop}><h2 id="heading">只读对象</h2><button data-initial-focus>关闭</button></ModalDialog>);
  const dialog = screen.getByRole('dialog');
  vi.spyOn(dialog, 'getBoundingClientRect').mockReturnValue(new DOMRect(100, 100, 400, 300));
  const pointer = (kind: string, x: number, y: number) => fireEvent(dialog, new MouseEvent(kind, { bubbles: true, clientX: x, clientY: y }));
  return { dialog, onClose, pointer };
}

describe('ModalDialog 按用途关闭', () => {
  it('默认不启用遮罩关闭，保护其他既有表单', () => {
    const { onClose, pointer } = setup(); pointer('pointerdown', 10, 10); pointer('pointerup', 10, 10);
    expect(onClose).not.toHaveBeenCalled();
  });
  it('只读显式允许遮罩，必须起止都在外部；拖出边界不关闭', () => {
    const { onClose, pointer } = setup(true);
    pointer('pointerdown', 150, 150); pointer('pointerup', 10, 10); expect(onClose).not.toHaveBeenCalled();
    pointer('pointerdown', 10, 10); pointer('pointerup', 150, 150); expect(onClose).not.toHaveBeenCalled();
    pointer('pointerdown', 10, 10); pointer('pointerup', 10, 10); expect(onClose).toHaveBeenCalledTimes(1);
  });
  it('初始焦点可指定；Esc交还拥有者决定，不直接取消表单', () => {
    const { dialog, onClose } = setup(); expect(screen.getByRole('button')).toHaveFocus();
    const event = new Event('cancel', { cancelable: true }); fireEvent(dialog, event);
    expect(onClose).toHaveBeenCalledTimes(1); expect(event.defaultPrevented).toBe(true); expect(dialog).toHaveAttribute('open');
  });
});
