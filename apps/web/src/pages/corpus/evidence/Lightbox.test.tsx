import { act, fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { parseWorkDetail } from './evidenceDecode';
import { detailJson, mediaHandle, slotJson, workRef } from './evidenceFixtures';
import { Lightbox } from './Lightbox';
import { mediaObjects, type MediaObject } from './mediaObjects';

const ref = workRef(1);

function objects(count = 3, extra: ReturnType<typeof slotJson>[] = []): MediaObject[] {
  const slots = [...Array.from({ length: count }, (_, index) => slotJson(index + 1, ref, { ordinal: index + 1 })), ...extra];
  return mediaObjects(parseWorkDetail(detailJson(1, { slots })).inspector, null);
}

function setup(list: MediaObject[] = objects(), index = 0) {
  const onIndex = vi.fn();
  const onClose = vi.fn();
  const view = render(<Lightbox objects={list} index={index} onIndex={onIndex} onClose={onClose} />);
  return { ...view, onIndex, onClose };
}

/** 键盘事件从对话框内部的控件冒泡上来（焦点本来就在对话框里）。 */
const keys = () => screen.getByRole('button', { name: '关闭查看器' });
const zoomText = () => screen.getByRole('group', { name: '缩放' }).querySelector('button:nth-child(2)')?.textContent;

describe('原尺寸查看器', () => {
  it('模态对话框：标题「用途 · 第 NN / MM 个」，副标题给出对象编号与采集序号，图注写字节与文字状态', () => {
    setup();
    const dialog = screen.getByRole('dialog', { name: /正文图片 · 第 01 \/ 03 个/ });
    expect(dialog).toBeInTheDocument();
    expect(screen.getByText(/^MED-[0-9A-F]{8} · 采集序号 1 · 平台排列顺序未经验证$/)).toBeInTheDocument();
    expect(screen.getByRole('img', { name: '正文图片 原尺寸本地副本' })).toHaveAttribute('src', mediaHandle(1));
    expect(screen.getByText('字节 已取得 · 文字 未处理')).toBeInTheDocument();
  });

  it('换图：上一张／下一张按钮与 ← → 键；到头禁用；单图模式不能前后翻', () => {
    const { onIndex } = setup(objects(), 1);
    fireEvent.click(screen.getByRole('button', { name: '上一张' }));
    expect(onIndex).toHaveBeenLastCalledWith(0);
    fireEvent.click(screen.getByRole('button', { name: '下一张' }));
    expect(onIndex).toHaveBeenLastCalledWith(2);
    fireEvent.keyDown(keys(), { key: 'ArrowLeft' });
    expect(onIndex).toHaveBeenLastCalledWith(0);
    fireEvent.keyDown(keys(), { key: 'ArrowRight' });
    expect(onIndex).toHaveBeenLastCalledWith(2);
  });

  it('到头的方向禁用；只有一个对象时前后都禁用', () => {
    const first = setup(objects(3), 0);
    expect(screen.getByRole('button', { name: '上一张' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '下一张' })).toBeEnabled();
    first.unmount();
    const last = setup(objects(3), 2);
    expect(screen.getByRole('button', { name: '下一张' })).toBeDisabled();
    last.unmount();
    setup(objects(1));
    expect(screen.getByRole('button', { name: '上一张' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '下一张' })).toBeDisabled();
  });

  it('缩放 1×–6×：按钮、+／-／0 键、双击、滚轮；缩小在 100% 禁用，放大在 600% 禁用', () => {
    setup();
    expect(zoomText()).toBe('100%');
    expect(screen.getByRole('button', { name: '缩小' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: '放大' }));
    expect(zoomText()).toBe('150%');
    fireEvent.keyDown(keys(), { key: '+' });
    expect(zoomText()).toBe('225%');
    fireEvent.keyDown(keys(), { key: '-' });
    expect(zoomText()).toBe('150%');
    fireEvent.keyDown(keys(), { key: '0' });
    expect(zoomText()).toBe('100%');
    const frame = screen.getByRole('img', { name: '正文图片 原尺寸本地副本' }).parentElement as HTMLElement;
    fireEvent.doubleClick(frame, { clientX: 10, clientY: 10 });
    expect(zoomText()).toBe('250%');
    expect(frame).toHaveAttribute('data-zoomed', 'true');
    fireEvent.doubleClick(frame);
    expect(zoomText()).toBe('100%');
    const wheel = new WheelEvent('wheel', { deltaY: -100, cancelable: true });
    act(() => {
      frame.dispatchEvent(wheel);
    });
    expect(wheel.defaultPrevented).toBe(true);
    expect(zoomText()).toBe('112%');
    for (let step = 0; step < 12; step += 1) fireEvent.click(screen.getByRole('button', { name: '放大' }));
    expect(zoomText()).toBe('600%');
    expect(screen.getByRole('button', { name: '放大' })).toBeDisabled();
  });

  it('换一张图缩放归位', () => {
    const list = objects();
    const { rerender, onIndex, onClose } = setup(list);
    fireEvent.click(screen.getByRole('button', { name: '放大' }));
    expect(zoomText()).toBe('150%');
    rerender(<Lightbox objects={list} index={1} onIndex={onIndex} onClose={onClose} />);
    expect(zoomText()).toBe('100%');
  });

  it('换图之后焦点仍在刚按下的按钮上，且对话框是同一个节点；最后关闭时焦点回到打开它的卡片', () => {
    const list = objects();
    function Host() {
      const [index, setIndex] = useState(0);
      const [open, setOpen] = useState(false);
      return (
        <>
          <button type="button" onClick={() => setOpen(true)}>
            打开查看器
          </button>
          {open ? <Lightbox objects={list} index={index} onIndex={setIndex} onClose={() => setOpen(false)} /> : null}
        </>
      );
    }
    render(<Host />);
    const opener = screen.getByRole('button', { name: '打开查看器' });
    opener.focus();
    fireEvent.click(opener);
    const dialog = screen.getByRole('dialog');
    const next = screen.getByRole('button', { name: '下一张' });
    next.focus();
    fireEvent.click(next);
    expect(screen.getByRole('dialog', { name: /第 02 \/ 03 个/ })).toBe(dialog);
    expect(screen.getByRole('button', { name: '下一张' })).toBe(next);
    expect(next).toHaveFocus();
    fireEvent.click(screen.getByRole('button', { name: '关闭查看器' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(opener).toHaveFocus();
  });

  it('关闭：Esc（对话框取消事件）、关闭按钮、点舞台空白；点图片本身不关闭', () => {
    const { onClose } = setup();
    fireEvent(screen.getByRole('dialog'), new Event('cancel', { cancelable: true }));
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: '关闭查看器' }));
    expect(onClose).toHaveBeenCalledTimes(2);
    fireEvent.click(screen.getByRole('img', { name: '正文图片 原尺寸本地副本' }));
    expect(onClose).toHaveBeenCalledTimes(2);
    const stage = screen.getByRole('img', { name: '正文图片 原尺寸本地副本' }).closest('figure')!.parentElement!;
    fireEvent.pointerDown(stage); fireEvent.pointerUp(stage);
    expect(onClose).toHaveBeenCalledTimes(3);
  });

  it('打开期间页面不滚动，关闭后恢复', () => {
    document.body.style.overflow = 'auto';
    const { unmount } = setup();
    expect(document.body.style.overflow).toBe('hidden');
    unmount();
    expect(document.body.style.overflow).toBe('auto');
    document.body.style.overflow = '';
  });

  it('没有可内联副本：不渲染图片，图注写原因', () => {
    setup(objects(1, [slotJson(2, ref, { ordinal: 2, handle: null, bytesState: 'NOT_OBSERVED' })]), 1);
    expect(screen.queryByRole('img', { name: /原尺寸本地副本/ })).toBeNull();
    expect(screen.getByText('这个对象没有可内联的本地副本：尚未形成观察')).toBeInTheDocument();
  });
});

it('查看器区分读取/失败；换图重新读取，控制按钮和焦点不重建', () => {
  const { rerender, onIndex, onClose } = setup();
  expect(screen.getByRole('status')).toHaveTextContent('正在读取本地副本');
  fireEvent.error(screen.getByRole('img', { name: '正文图片 原尺寸本地副本' }));
  expect(screen.getByRole('status')).toHaveTextContent('本地副本读取失败');
  expect(screen.queryByRole('img')).toBeNull();
  const next = screen.getByRole('button', { name: '下一张' }); next.focus();
  rerender(<Lightbox objects={objects()} index={1} onIndex={onIndex} onClose={onClose} />);
  expect(screen.getByRole('status')).toHaveTextContent('正在读取本地副本');
  expect(screen.getByRole('button', { name: '下一张' })).toBe(next);
  expect(next).toHaveFocus();
  fireEvent.load(screen.getByRole('img', { name: '正文图片 原尺寸本地副本' }));
  expect(screen.queryByRole('status')).toBeNull();
});
it('从图片拖到舞台空白不会关闭；只有空白按下和抬起才关闭', () => {
  const { onClose } = setup();
  const image = screen.getByRole('img', { name: '正文图片 原尺寸本地副本' });
  const stage = image.closest('figure')!.parentElement!;
  fireEvent.pointerDown(image); fireEvent.pointerUp(stage);
  expect(onClose).not.toHaveBeenCalled();
  fireEvent.pointerDown(stage); fireEvent.pointerCancel(stage); fireEvent.pointerUp(stage);
  expect(onClose).not.toHaveBeenCalled();
});
