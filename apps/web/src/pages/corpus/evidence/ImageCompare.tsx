import { useState, type Ref } from 'react';
import { lineRole, imageSummary, processingState } from './dossierPresent';
import type { DossierLine, DossierMedia } from './dossierTypes';
import styles from './Dossier.module.css';

function Highlight({ box }: { box: NonNullable<DossierLine['bbox']> }) {
  const [left, top, right, bottom] = box;
  return <span className={styles.highlight} style={{ left: `${left * 100}%`, top: `${top * 100}%`, width: `${(right - left) * 100}%`, height: `${(bottom - top) * 100}%` }} />;
}

/**
 * 一张图的「原图与识别对照」：左图右文（容器窄时上下排）。点识别行在原图上叠框（坐标须在 0–1，否则不画）；
 * 已排除的水印、界面与不清文字折叠；原图只接受受控本地句柄，读取失败就地说明，不影响其余图片。
 */
export function ImageCompare({ media, onOpenImage, register }: { media: DossierMedia; onOpenImage: (media: DossierMedia) => void; register: Ref<HTMLDetailsElement> }) {
  const [failed, setFailed] = useState(false);
  const [highlight, setHighlight] = useState<DossierLine['bbox']>(null);
  const layering = media.layering;
  const image = media.originalUrl;
  const roleOf = (line: DossierLine) => layering?.retained[line.lineRef] ?? layering?.excluded[line.lineRef] ?? 'uncertain';
  const kept = layering?.lines.filter(line => layering.excluded[line.lineRef] === undefined) ?? [];
  const excluded = layering?.lines.filter(line => layering.excluded[line.lineRef] !== undefined) ?? [];
  const row = (line: DossierLine) => {
    const role = roleOf(line);
    return (
      <button key={line.lineRef} type="button" className={styles.line} data-role={role} title="点击在原图标出位置" onClick={() => setHighlight(image && !failed ? line.bbox : null)}>
        {`${lineRole(role)} · ${line.text}`}
      </button>
    );
  };
  return (
    <details ref={register} className={styles.image}>
      <summary>{imageSummary(media)}</summary>
      <div className={styles.compare}>
        <div className={styles.visual}>
          {image && !failed ? (
            <img src={image} alt={`图片 ${media.slotOrdinal ?? ''} 原图对照`} loading="lazy" onError={() => setFailed(true)} />
          ) : image ? (
            <p>本地原图暂时不可读，请在材料页查看获取状态。</p>
          ) : (
            <p className={styles.note}>本地原图尚不可用。可在“材料”页查看或补采。</p>
          )}
          {highlight ? <Highlight box={highlight} /> : null}
        </div>
        <div className={styles.lines}>
          {kept.map(row)}
          {kept.length === 0 ? <p>尚无可对照的识别行。</p> : null}
          {excluded.length > 0 ? (
            <details>
              <summary>已排除的水印、界面与不清文字</summary>
              {excluded.map(row)}
            </details>
          ) : null}
        </div>
      </div>
      {image ? (
        <button type="button" onClick={() => onOpenImage(media)}>
          放大查看原图
        </button>
      ) : null}
      {media.otherProcessing ? <p className={styles.note}>{`另有处理结果：${processingState(media.otherProcessing.state)}。当前展示所选可读版本。`}</p> : null}
      <details className={styles.disclosure}>
        <summary>原始识别文字（未筛选）</summary>
        <pre className={styles.pre}>{media.researchText ?? media.displayText ?? '未取得'}</pre>
      </details>
      {layering ? (
        <details className={styles.disclosure}>
          <summary>来源与分层版本</summary>
          <pre className={styles.pre}>
            {JSON.stringify(
              {
                layoutRef: layering.layoutRef,
                layeringRef: layering.layeringRef,
                layerVersion: layering.layerVersion,
                imageWidth: layering.imageWidth,
                imageHeight: layering.imageHeight,
                engineVersion: layering.engineVersion,
                selectionBasis: media.selectionBasis,
                slotKey: media.slotKey,
              },
              null,
              2,
            )}
          </pre>
        </details>
      ) : null}
    </details>
  );
}
