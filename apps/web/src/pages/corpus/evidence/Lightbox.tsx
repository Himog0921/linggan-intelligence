import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { stateMeta } from './evidencePresent';
import { positionText, type ViewerObject } from './mediaObjects';
import { useZoomPan } from './useZoomPan';
import styles from './Lightbox.module.css';

interface LightboxProps {
  /** 可浏览的对象；单图模式（底稿里的「放大查看原图」）只有一个，不能前后翻。 */
  objects: readonly ViewerObject[];
  index: number;
  onIndex: (index: number) => void;
  onClose: () => void;
}

function caption(object: ViewerObject): string {
  return object.assetUrl
    ? `字节 ${stateMeta(object.bytesState)[0]} · 文字 ${object.derivedState ? stateMeta(object.derivedState)[0] : '未处理'}`
    : `这个对象没有可内联的本地副本：${stateMeta(object.bytesState)[0]}`;
}

function Viewer({ object, total, titleId, onIndex, onClose }: { object: ViewerObject; total: number; titleId: string; onIndex: (index: number) => void; onClose: () => void }) {
  const frame = useRef<HTMLDivElement>(null);
  const image = useRef<HTMLImageElement>(null);
  const zoom = useZoomPan(frame, image, object.index);
  const mediaKey = `${object.index}:${object.assetUrl ?? ''}`;
  const [imageState, setImageState] = useState<{ key: string; phase: 'loading' | 'loaded' | 'failed' }>({ key: mediaKey, phase: 'loading' });
  if (imageState.key !== mediaKey) setImageState({ key: mediaKey, phase: 'loading' });
  const phase = imageState.key === mediaKey ? imageState.phase : 'loading';
  const stagePress = useRef(false);
  const previous = object.index > 0 && total > 1;
  const next = object.index < total - 1;

  function onKeyDown(event: KeyboardEvent) {
    const actions: Record<string, () => void> = {
      ArrowLeft: () => previous && onIndex(object.index - 1),
      ArrowRight: () => next && onIndex(object.index + 1),
      '+': zoom.zoomIn,
      '=': zoom.zoomIn,
      '-': zoom.zoomOut,
      _: zoom.zoomOut,
      '0': zoom.reset,
    };
    const action = actions[event.key];
    if (!action) return;
    event.preventDefault();
    action();
  }

  const subtitle = `${object.objectRef ? `MED-${object.objectRef.slice(0, 8).toUpperCase()} · ` : '本地对象编号未表达 · '}采集序号 ${object.ordinal} · 平台排列顺序未经验证`;
  return (
    <div className={styles.viewer} onKeyDown={onKeyDown}>
      <header className={styles.head}>
        <div>
          <strong id={titleId}>{`${object.label} · 第 ${positionText(object.index, total).replace(' / ', ' / ')} 个`}</strong>
          <span className={styles.subtitle}>{subtitle}</span>
        </div>
        <div className={styles.tools}>
          <div className={styles.zoom} role="group" aria-label="缩放">
            <button type="button" aria-label="缩小" disabled={!zoom.canZoomOut} onClick={zoom.zoomOut}>
              <span aria-hidden="true">−</span>
            </button>
            <button type="button" className={styles.zoomValue} onClick={zoom.reset}>
              {`${Math.round(zoom.zoom * 100)}%`}
            </button>
            <button type="button" aria-label="放大" disabled={!zoom.canZoomIn} onClick={zoom.zoomIn}>
              <span aria-hidden="true">+</span>
            </button>
          </div>
          <button type="button" className={styles.close} aria-label="关闭查看器" onClick={onClose}>
            <span aria-hidden="true">×</span>
          </button>
        </div>
      </header>
      <div className={styles.stage}
        onPointerDown={event => { stagePress.current = event.target === event.currentTarget; }}
        onPointerCancel={() => { stagePress.current = false; }}
        onPointerUp={event => { if (stagePress.current && event.target === event.currentTarget) onClose(); stagePress.current = false; }}>
        <button type="button" className={styles.step} aria-label="上一张" disabled={!previous} onClick={() => onIndex(object.index - 1)}>
          <span aria-hidden="true">←</span>
        </button>
        <figure className={styles.figure}>
          <div
            ref={frame}
            className={styles.frame}
            data-zoomed={zoom.zoom > 1}
            data-grabbing={zoom.grabbing}
            onDoubleClick={zoom.onDoubleClick}
            onPointerDown={zoom.onPointerDown}
            onPointerMove={zoom.onPointerMove}
            onPointerUp={zoom.onPointerEnd}
            onPointerCancel={zoom.onPointerEnd}
          >
            {object.assetUrl && phase !== 'failed' ? (
              <img key={object.index} ref={image} src={object.assetUrl} alt={`${object.label} 原尺寸本地副本`} onLoad={() => setImageState({ key: mediaKey, phase: 'loaded' })} onError={() => setImageState({ key: mediaKey, phase: 'failed' })} style={{ transform: `translate(${zoom.pan.x}px, ${zoom.pan.y}px) scale(${zoom.zoom})` }} />
            ) : null}
            {!object.assetUrl || phase !== 'loaded' ? <span className={styles.mediaState} role="status">{!object.assetUrl ? `未取得可内联副本 · ${stateMeta(object.bytesState)[0]}` : phase === 'failed' ? '本地副本读取失败；可切换其他对象或关闭查看器。' : '正在读取本地副本…'}</span> : null}
          </div>
          <figcaption>{caption(object)}</figcaption>
        </figure>
        <button type="button" className={styles.step} aria-label="下一张" disabled={!next} onClick={() => onIndex(object.index + 1)}>
          <span aria-hidden="true">→</span>
        </button>
      </div>
    </div>
  );
}

/**
 * 原尺寸查看器：模态原生 `<dialog>`，Tab 只在它自己的控件间循环，Esc 关闭，关闭后焦点回到打开它的卡片。
 * 图片 `contain` 不裁切；缩放 1×–6×，平移被夹在溢出范围内。换一张图缩放归位，但对话框与按钮是同一批节点——
 * 键盘用户按下「下一张」之后焦点仍在那个按钮上，最后关闭时才能回到打开它的卡片。
 */
export function Lightbox({ objects, index, onIndex, onClose }: LightboxProps) {
  const titleId = useId();
  const object = objects[Math.min(objects.length - 1, Math.max(0, index))];
  // 打开期间页面不滚动。
  useEffect(() => {
    const before = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    return () => {
      document.body.style.overflow = before;
    };
  }, []);
  if (!object) return null;
  return (
    <ModalDialog labelledBy={titleId} className={styles.lightbox} onClose={onClose}>
      <Viewer object={object} total={objects.length} titleId={titleId} onIndex={onIndex} onClose={onClose} />
    </ModalDialog>
  );
}
