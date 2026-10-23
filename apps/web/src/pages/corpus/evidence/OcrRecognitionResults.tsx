import type { ReactNode } from 'react';
import { useState } from 'react';
import type { OcrRecognitionImage, OcrRecognitionRegion, OcrRecognitionRun } from './ocrRecognitionTypes';
import { ManualOcrAdoption } from './ManualOcrAdoption';
import styles from './OcrRecognition.module.css';

export type OcrSourceOpener = (image: OcrRecognitionImage, opener: HTMLElement) => void;
const pageLabel = (image: OcrRecognitionImage) => image.ordinal === null ? '顺序未知的图片' : '第 ' + image.ordinal + ' 图';
function Source({ image, onOpen, children = '核对原图' }: { image: OcrRecognitionImage; onOpen?: OcrSourceOpener; children?: ReactNode }) {
  if (!image.originalUrl) return <small>原图当前不可用</small>;
  return onOpen && image.slotKey ? <button type="button" className={styles.source} onClick={event => onOpen(image, event.currentTarget)}>{children}</button>
    : <a className={styles.source} href={image.originalUrl} target="_blank" rel="noreferrer">{children}</a>;
}
function RegionText({ region }: { region: OcrRecognitionRegion }) {
  if (region.spans.length === 0) return region.status === 'pending'
    ? <mark title={region.reason ?? '识别内容尚待核验'}>{region.text}<small>〔待核验〕</small></mark> : <>{region.text}</>;
  const points = Array.from(region.text); const fragments: ReactNode[] = []; let cursor = 0;
  for (const span of region.spans) {
    if (span.start > cursor) fragments.push(points.slice(cursor, span.start).join(''));
    if (span.status !== 'excluded') {
      const text = points.slice(span.start, span.end).join('');
      fragments.push(span.status === 'pending' ? <mark key={span.start} title={span.reason ?? '待核验'}>{text}<small>〔待核验〕</small></mark> : text);
    } else fragments.push(<small key={span.start} className={styles.omitted}>〔疑似干扰文字，见原始识别〕</small>);
    cursor = span.end;
  }
  fragments.push(points.slice(cursor).join(''));
  return <>{fragments}</>;
}
function PageText({ image, onOpen, compare, coverOther = false, workRef, onSaved }: { image: OcrRecognitionImage; onOpen?: OcrSourceOpener; compare: boolean; coverOther?: boolean; workRef?: string; onSaved?: () => void }) {
  const regions = image.regions.filter(region => region.status !== 'excluded' && (!coverOther || region.role !== 'main_title'));
  return <article className={styles.page} data-ocr-image={image.imageRef}>
    <div className={styles.pageHeading}><h5>{coverOther ? '封面其他文字' : pageLabel(image)}</h5><Source image={image} onOpen={onOpen}/></div>
    {compare && image.originalUrl ? <img className={styles.original} src={image.originalUrl} alt={pageLabel(image) + '原图'}/> : null}
    {image.state === 'unattempted' || image.state === 'pending' ? <p className={styles.status}>此图尚未识别。</p>
      : image.state === 'failed' || image.state === 'unknown' ? <p className={styles.status}>此图结果未成功确认，已停止后续识别。</p>
        : regions.length === 0 ? <p className={styles.status}>{image.regions.length ? '识别到内容，暂无可采用文字；请展开原始识别。' : '本次尚无可读文字结果。'}</p>
          : regions.map(region => <p key={region.regionRef} className={styles.region} data-status={region.status}><RegionText region={region}/></p>)}
    {compare ? <details><summary>本图原始区块与状态</summary>{image.regions.map(region => <div key={region.regionRef} className={styles.rawRegion}>
      <small>{region.role} · {region.status === 'adopted' ? '当前采用，未人工核验' : region.status === 'pending' ? '待核验' : '未采用'} · {region.bbox === null ? '坐标未知，可回整图' : '归一化原图坐标 ' + region.bbox.join(', ')}</small>
      <p>{region.text || '〔原区块无可读文字〕'}</p>{region.reason ? <small>{region.reason}</small> : null}
    </div>)}</details> : null}
    {workRef && onSaved ? <ManualOcrAdoption workRef={workRef} image={image} onSaved={onSaved}/> : null}
  </article>;
}
export function OcrRecognitionResults({ run, onOpen, onSaved }: { run: OcrRecognitionRun; onOpen?: OcrSourceOpener; onSaved?: () => void }) {
  const [compare, setCompare] = useState(false);
  const cover = run.images.find(image => image.role === 'cover');
  const pages = run.images.filter(image => image.role === 'page');
  const unknown = run.images.filter(image => image.role === 'unknown');
  const pending = run.images.flatMap(image => image.regions.flatMap(region => {
    const spans = region.spans.filter(span => span.status === 'pending');
    return spans.length ? spans.map(span => ({ image, text: Array.from(region.text).slice(span.start, span.end).join(''), reason: span.reason, comparisonText: span.comparisonText }))
      : region.status === 'pending' ? [{ image, text: region.text, reason: region.reason, comparisonText: null }] : [];
  }));
  return <section className={styles.results} aria-label="图片文字识别结果" tabIndex={-1}>
    <header className={styles.resultHeading}><div><h4>图片文字</h4><p>{run.modelLabel ?? '模型未知'} · {run.state === 'succeeded' ? '识别完成' : '已有部分结果'}</p></div><span>模型识别 · 未人工核验</span></header>
    <div className={styles.resultBlock}><b>封面标题</b><p className={styles.coverTitle}>{run.coverTitle ?? (run.coverTitleStatus === 'absent' ? '首图未选出主标题' : '尚无确认的封面标题')}</p>
      <div className={styles.pageHeading}><small>{run.coverTitleStatus === 'pending' ? '标题候选待核验' : '仅从首图选择；不改写平台标题'}</small>{cover ? <Source image={cover} onOpen={onOpen}/> : null}</div></div>
    <div className={styles.readModes} aria-label="图片文字阅读方式"><button type="button" aria-pressed={!compare} onClick={() => setCompare(false)}>合并阅读</button><button type="button" aria-pressed={compare} onClick={() => setCompare(true)}>逐图对照</button></div>
    {cover && cover.regions.some(region => region.role !== 'main_title') ? <PageText image={cover} onOpen={onOpen} compare={compare} coverOther workRef={run.workRef} onSaved={onSaved}/> : null}
    <div className={styles.resultBlock}><b>图片正文</b>{pages.length ? pages.map(image => <PageText key={image.imageRef} image={image} onOpen={onOpen} compare={compare} workRef={run.workRef} onSaved={onSaved}/>) : <p className={styles.status}>尚无可读的内页正文。</p>}</div>
    {unknown.length ? <div className={styles.resultBlock}><b>顺序未知 · 单图保留</b><small>未将这些图片推定为首图或拼入有序正文。</small>{unknown.map(image => <PageText key={image.imageRef} image={image} onOpen={onOpen} compare={compare} workRef={run.workRef} onSaved={onSaved}/>)}</div> : null}
    {pending.length ? <details className={styles.pending}><summary>待核验内容 · {pending.length} 处</summary>{pending.map((entry, index) => <div key={index}><p>{entry.text || '不可读区域'}</p>{entry.comparisonText ? <p>已有同图对照原文：{entry.comparisonText}</p> : null}<small>{entry.reason ?? '需对照原图核验'} · {pageLabel(entry.image)}</small><Source image={entry.image} onOpen={onOpen}/></div>)}</details> : null}
    <details className={styles.receipts}><summary>原始识别与运行回执</summary><p className={styles.status}>原文按原样保留。估算费用依据调用账本；实际扣款未提供时不记为零。</p>
      {run.images.map(image => <details key={image.imageRef}><summary>{pageLabel(image)} · {image.rawJson === null ? '尚无原始返回' : '原始 JSON 与用量'}</summary>
        <dl className={styles.receiptFacts}><dt>结束原因</dt><dd>{image.finishReason ?? '未提供'}</dd><dt>输入 token</dt><dd>{image.usage?.inputTokens ?? '未提供'}</dd><dt>输出 token</dt><dd>{image.usage?.outputTokens ?? '未提供'}</dd><dt>实测 token</dt><dd>{image.usage?.measuredTokens ?? '未提供'}</dd><dt>估算费用 USD</dt><dd>{image.estimatedCostUsd ?? '未提供'}</dd><dt>实际扣款</dt><dd>未提供</dd><dt>原始结果 SHA-256</dt><dd>{image.rawResultSha256 ?? '未提供'}</dd><dt>调用引用</dt><dd>{image.invocationRef ?? '未发出或未确认'}</dd></dl>
        <Source image={image} onOpen={onOpen}/>{image.rawJson === null ? <p>当前没有可读取的原始返回。</p> : <pre aria-label={pageLabel(image) + '原始 JSON'}>{image.rawJson}</pre>}
      </details>)}
    </details>
  </section>;
}
