import { useState } from 'react';
import { DossierBlocks } from './DossierBlocks';
import { OcrReadingReview } from './OcrReadingReview';
import type { OcrRecognitionImage, OcrRecognitionRun } from './ocrRecognitionTypes';
import styles from './OcrRecognition.module.css';
export type OcrSourceOpener = (image: OcrRecognitionImage, opener: HTMLElement) => void;
const pageLabel = (image: OcrRecognitionImage) => image.ordinal === null ? image.role === 'cover' ? '封面 · 页码未知' : '页码未知的图片' : `第 ${image.ordinal} 页`;
function Source({ image, onOpen }: { image: OcrRecognitionImage; onOpen?: OcrSourceOpener }) {
  if (!image.originalUrl) return <small>原图当前不可用</small>;
  return onOpen && image.slotKey ? <button type="button" className={styles.source} onClick={event => onOpen(image, event.currentTarget)}>核对原图</button> : <a className={styles.source} href={image.originalUrl} target="_blank" rel="noreferrer">核对原图</a>;
}
export function OcrRecognitionResults({ run, domain, onOpen, onSaved }: { run: OcrRecognitionRun; domain: string; onOpen?: OcrSourceOpener; onSaved: () => void }) {
  const [compare, setCompare] = useState(false);
  return <section className={styles.results} aria-label="图片文字识别结果" tabIndex={-1}>
    <header className={styles.resultHeading}><div><h4>候选读本</h4><p>{run.modelLabel ?? '模型未知'} · {run.state === 'succeeded' ? '识读完成' : '已有部分结果'}</p></div></header>
    <p className={styles.status}>重新识读只生成候选。对照原图后，可在每张图片下方核对、采用或修正。</p>
    <div className={styles.readModes} aria-label="候选读本阅读方式"><button type="button" aria-pressed={!compare} onClick={() => setCompare(false)}>连续阅读</button><button type="button" aria-pressed={compare} onClick={() => setCompare(true)}>逐图对照</button></div>
    {run.images.map(image => <article key={image.imageRef} className={styles.page} data-ocr-image={image.imageRef}>
      <div className={styles.pageHeading}><h5>{pageLabel(image)}</h5><Source image={image} onOpen={onOpen} /></div>
      {compare && image.originalUrl ? <img className={styles.original} src={image.originalUrl} alt={`${pageLabel(image)}原图`} /> : null}
      {image.state === 'unattempted' || image.state === 'pending' ? <p className={styles.status}>此图尚未识读。</p> : image.state === 'failed' || image.state === 'unknown' ? <p className={styles.status}>此图结果未成功确认。</p> : image.blocks.length === 0 ? <p className={styles.status}>本次尚无可读文字结果，不能据此判断图中没有文字。</p> : null}
      <DossierBlocks blocks={image.blocks} issues={image.issues} />
      <OcrReadingReview image={image} work={run.workRef} domain={domain} onSaved={onSaved} />
    </article>)}
    <details className={styles.receipts}><summary>原始识别与运行回执</summary>
      {run.images.map(image => <details key={image.imageRef}><summary>{pageLabel(image)} · {image.rawJson === null ? '尚无原始返回' : '原始 JSON 与用量'}</summary>
        <dl className={styles.receiptFacts}><dt>结束原因</dt><dd>{image.finishReason ?? '未提供'}</dd><dt>输入 token</dt><dd>{image.usage?.inputTokens ?? '未提供'}</dd><dt>输出 token</dt><dd>{image.usage?.outputTokens ?? '未提供'}</dd><dt>估算费用 USD</dt><dd>{image.estimatedCostUsd ?? '未提供'}</dd><dt>候选版本</dt><dd>{image.candidateRevisionRef ?? '尚未生成'}</dd><dt>原始结果 SHA-256</dt><dd>{image.rawResultSha256 ?? '未提供'}</dd><dt>调用引用</dt><dd>{image.invocationRef ?? '未发出或未确认'}</dd></dl>
        {image.rawJson === null ? <p>当前没有可读取的原始返回。</p> : <pre aria-label={`${pageLabel(image)}原始 JSON`}>{image.rawJson}</pre>}
      </details>)}
    </details>
  </section>;
}
