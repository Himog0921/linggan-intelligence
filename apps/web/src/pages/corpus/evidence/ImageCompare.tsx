import { useState, type Ref } from 'react';
import { blockText, materialLabel } from './dossierPresent';
import type { DossierBlock, DossierMaterial } from './dossierTypes';
import styles from './Dossier.module.css';
function Highlight({ box }: { box: NonNullable<DossierBlock['bbox']> }) {
  const [x, y, width, height] = box;
  return <span className={styles.highlight} style={{ left: `${x * 100}%`, top: `${y * 100}%`, width: `${width * 100}%`, height: `${height * 100}%` }} />;
}
export function ImageCompare({ material, onOpenImage, register, onReadable }: { material: DossierMaterial; onOpenImage?: (material: DossierMaterial) => void; register?: Ref<HTMLDetailsElement>; onReadable?: (readable: boolean) => void }) {
  const [failed, setFailed] = useState(false); const [highlight, setHighlight] = useState<DossierBlock['bbox']>(null);
  const image = material.originalUrl;
  return <details ref={register} className={styles.image}><summary>对照原图 · {materialLabel(material)}</summary>
    <div className={styles.compare}>
      <div className={styles.visual}>{image && !failed ? <img src={image} alt={`${materialLabel(material)}原图`} loading="lazy" onLoad={() => onReadable?.(true)} onError={() => { setFailed(true); setHighlight(null); onReadable?.(false); }} /> : <p>{image ? '本地原图暂时不可读，请在材料页查看获取状态。' : '本地原图尚未取得。'}</p>}{highlight ? <Highlight box={highlight} /> : null}</div>
      <div>{material.blocks.map((block, index) => <button key={block.ref} type="button" className={styles.line} disabled={!block.bbox || !image || failed} onClick={() => setHighlight(block.bbox)}>{`${index + 1} · ${blockText(block)}`}{!block.bbox ? '（无位置坐标）' : ''}</button>)}{material.blocks.length === 0 ? <p>尚无可对照的读本区块。</p> : null}</div>
    </div>{image && !failed ? onOpenImage ? <button type="button" onClick={() => onOpenImage(material)}>放大查看原图</button> : <a href={image} target="_blank" rel="noreferrer">放大查看原图</a> : null}
  </details>;
}
