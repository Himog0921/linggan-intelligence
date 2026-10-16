import { createEvent, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { parseWorkDetail } from './evidenceDecode';
import { detailJson, mediaHandle, slotJson, workRef } from './evidenceFixtures';
import { mediaObjects, type MediaObject } from './mediaObjects';
import { MediaRail } from './MediaRail';

const ref = workRef(1);

function objects(options: { count?: number; extra?: ReturnType<typeof slotJson>[]; derivatives?: unknown[]; match?: string } = {}): MediaObject[] {
  const slots = [...Array.from({ length: options.count ?? 3 }, (_, index) => slotJson(index + 1, ref, { ordinal: index + 1 })), ...(options.extra ?? [])];
  const inspector = parseWorkDetail(detailJson(1, { slots, derivatives: options.derivatives ?? [] })).inspector;
  return mediaObjects(inspector, options.match ? { slotKey: options.match } : null);
}

function stubLayout() {
  vi.spyOn(HTMLElement.prototype, 'offsetLeft', 'get').mockImplementation(function (this: HTMLElement) {
    return Number(this.getAttribute('data-index') ?? 0) * 100;
  });
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('媒体轨道', () => {
  it('说明采集顺序；每张卡是按钮：图片、序号、文字处理状态；机器编号只放 title', () => {
    const onOpen = vi.fn();
    render(<MediaRail objects={objects({ derivatives: [{ kind: 'ocr_text', state: 'ACQUIRED', slotKey: `xhs:${ref}:image:2`, displayText: '字' }] })} onOpen={onOpen} />);
    expect(screen.getByText('03 个本地媒体对象')).toBeInTheDocument();
    expect(screen.getByText('采集顺序 · 平台排列顺序未经验证')).toBeInTheDocument();
    const group = screen.getByRole('group', { name: '本地媒体对象，左右方向键切换' });
    const cards = within(group).getAllByRole('button');
    expect(cards).toHaveLength(3);
    expect(within(cards[0]!).getByRole('img', { name: '正文图片 第 1 个本地媒体对象' })).toHaveAttribute('src', mediaHandle(1));
    expect(within(cards[0]!).getByText('01 / 03')).toBeInTheDocument();
    expect(within(cards[0]!).getByText('文字未处理')).toBeInTheDocument();
    expect(within(cards[1]!).getByText('文字 已取得')).toBeInTheDocument();
    expect(cards[0]).toHaveAttribute('title', expect.stringMatching(/MED-[0-9A-F]{8}/));
    expect(screen.queryByText(/MED-/)).toBeNull();
  });

  it('点卡片打开查看器，并把卡片交给查看器作为关闭后的焦点落点', () => {
    const onOpen = vi.fn();
    render(<MediaRail objects={objects()} onOpen={onOpen} />);
    const card = screen.getAllByRole('button').find(button => button.getAttribute('data-index') === '1');
    fireEvent.click(card!);
    expect(onOpen).toHaveBeenCalledWith(1, card);
  });

  it('不可内联的类型说明原因；视频与没有本地副本的对象写字节状态词；一张图读取失败只在原位提示', () => {
    render(
      <MediaRail
        objects={objects({
          count: 1,
          extra: [
            slotJson(2, ref, { ordinal: 2, deliveryState: 'UNSAFE_MIME', mime: 'image/svg+xml' }),
            slotJson(3, ref, { ordinal: 3, purpose: 'video', mime: 'video/mp4' }),
            slotJson(4, ref, { ordinal: 4, handle: null, bytesState: 'NOT_OBSERVED' }),
          ],
        })}
        onOpen={() => undefined}
      />,
    );
    expect(screen.getByText('本地副本已验证，但 image/svg+xml 不在可内联集合中。')).toBeInTheDocument();
    expect(screen.getByText('已取得')).toBeInTheDocument();
    expect(screen.getByText('尚未形成观察')).toBeInTheDocument();
    fireEvent.error(screen.getByRole('img', { name: '正文图片 第 1 个本地媒体对象' }));
    expect(screen.getByText('这个对象的本地副本读取失败；其余对象不受影响。')).toBeInTheDocument();
    expect(screen.queryByRole('img', { name: '正文图片 第 1 个本地媒体对象' })).toBeNull();
    expect(screen.getByText('尚未形成观察')).toBeInTheDocument();
  });

  it('「跳到命中」只在原声来自某个槽位时出现，点它把那张卡滚入并聚焦', () => {
    stubLayout();
    const scrollTo = vi.fn();
    Object.defineProperty(HTMLElement.prototype, 'scrollTo', { configurable: true, value: scrollTo });
    const { rerender } = render(<MediaRail objects={objects()} onOpen={() => undefined} />);
    expect(screen.queryByRole('button', { name: '跳到命中' })).toBeNull();
    rerender(<MediaRail objects={objects({ match: `xhs:${ref}:image:3` })} onOpen={() => undefined} />);
    fireEvent.click(screen.getByRole('button', { name: '跳到命中' }));
    expect(scrollTo).toHaveBeenCalledWith({ left: 200, behavior: 'smooth' });
    expect(document.activeElement).toHaveAttribute('data-index', '2');
    delete (HTMLElement.prototype as { scrollTo?: unknown }).scrollTo;
  });

  it('键盘 ← → Home End 与上一个／下一个按钮滚动轨道；到边按钮禁用', () => {
    stubLayout();
    const scrollTo = vi.fn();
    Object.defineProperty(HTMLElement.prototype, 'scrollTo', { configurable: true, value: scrollTo });
    render(<MediaRail objects={objects()} onOpen={() => undefined} />);
    const viewport = screen.getByRole('group', { name: '本地媒体对象，左右方向键切换' });
    expect(screen.getByRole('button', { name: '上一个媒体对象' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '下一个媒体对象' })).toBeEnabled();
    fireEvent.keyDown(viewport, { key: 'ArrowRight' });
    expect(scrollTo).toHaveBeenLastCalledWith({ left: 100, behavior: 'smooth' });
    fireEvent.keyDown(viewport, { key: 'End' });
    expect(scrollTo).toHaveBeenLastCalledWith({ left: 200, behavior: 'smooth' });
    fireEvent.click(screen.getByRole('button', { name: '下一个媒体对象' }));
    expect(scrollTo).toHaveBeenLastCalledWith({ left: 100, behavior: 'smooth' });
    const unrelated = createEvent.keyDown(viewport, { key: 'a' });
    fireEvent(viewport, unrelated);
    expect(unrelated.defaultPrevented).toBe(false);
    delete (HTMLElement.prototype as { scrollTo?: unknown }).scrollTo;
  });

  it('减少动态效果时直接跳，不平滑滚动', () => {
    stubLayout();
    vi.stubGlobal('matchMedia', (query: string) => ({ matches: query.includes('prefers-reduced-motion'), addEventListener: () => undefined, removeEventListener: () => undefined }));
    const scrollTo = vi.fn();
    Object.defineProperty(HTMLElement.prototype, 'scrollTo', { configurable: true, value: scrollTo });
    render(<MediaRail objects={objects()} onOpen={() => undefined} />);
    fireEvent.keyDown(screen.getByRole('group', { name: '本地媒体对象，左右方向键切换' }), { key: 'ArrowRight' });
    expect(scrollTo).toHaveBeenLastCalledWith({ left: 100, behavior: 'auto' });
    delete (HTMLElement.prototype as { scrollTo?: unknown }).scrollTo;
  });

  it('拖拽：位移超过 4px 才进入拖拽；拖完不当作点击；纯点击仍是点击', () => {
    const onOpen = vi.fn();
    render(<MediaRail objects={objects()} onOpen={onOpen} />);
    const viewport = screen.getByRole('group', { name: '本地媒体对象，左右方向键切换' });
    const card = screen.getAllByRole('button').find(button => button.getAttribute('data-index') === '0') as HTMLElement;
    fireEvent.pointerDown(viewport, { pointerType: 'mouse', button: 0, clientX: 100, pointerId: 1 });
    fireEvent.pointerMove(viewport, { clientX: 102, pointerId: 1 });
    expect(viewport).toHaveAttribute('data-grabbing', 'false');
    fireEvent.pointerMove(viewport, { clientX: 140, pointerId: 1 });
    expect(viewport).toHaveAttribute('data-grabbing', 'true');
    fireEvent.pointerUp(viewport, { pointerId: 1 });
    expect(viewport).toHaveAttribute('data-grabbing', 'false');
    fireEvent.click(card);
    expect(onOpen).not.toHaveBeenCalled();
    fireEvent.click(card);
    expect(onOpen).toHaveBeenCalledTimes(1);
  });

  it('触屏手指不走拖拽（原生滚动）；纵向滚轮转横向，到边让位', () => {
    render(<MediaRail objects={objects()} onOpen={() => undefined} />);
    const viewport = screen.getByRole('group', { name: '本地媒体对象，左右方向键切换' }) as HTMLElement;
    fireEvent.pointerDown(viewport, { pointerType: 'touch', button: 0, clientX: 100, pointerId: 2 });
    fireEvent.pointerMove(viewport, { clientX: 300, pointerId: 2 });
    expect(viewport).toHaveAttribute('data-grabbing', 'false');
    Object.defineProperty(viewport, 'scrollWidth', { configurable: true, value: 600 });
    Object.defineProperty(viewport, 'clientWidth', { configurable: true, value: 300 });
    viewport.scrollLeft = 10;
    const wheel = new WheelEvent('wheel', { deltaY: 40, cancelable: true });
    viewport.dispatchEvent(wheel);
    expect(wheel.defaultPrevented).toBe(true);
    expect(viewport.scrollLeft).toBe(50);
    viewport.scrollLeft = 0;
    const atStart = new WheelEvent('wheel', { deltaY: -40, cancelable: true });
    viewport.dispatchEvent(atStart);
    expect(atStart.defaultPrevented).toBe(false);
    const sideways = new WheelEvent('wheel', { deltaX: 40, deltaY: 5, cancelable: true });
    viewport.dispatchEvent(sideways);
    expect(sideways.defaultPrevented).toBe(false);
  });
});
