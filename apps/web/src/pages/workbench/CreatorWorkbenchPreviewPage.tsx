import { useEffect, useRef, useState } from 'react';
import { OpsFrame } from '../ops/OpsFrame';
import { NavIcon } from '../../shared/Sidebar';
import { ActionButton } from '../../shared/ActionButton';
import { CreatorWorkbenchGoldMasterAdapter, type CreatorWorkbenchGoldMasterHandle, type GoldMasterBundle, type GoldMasterMode } from './CreatorWorkbenchGoldMasterAdapter';
import styles from './CreatorWorkbenchPreviewPage.module.css';

const GOLDMASTER_REFERENCE_SHA = 'c1e564b38811bee179154ac3b2af1847abceab1fa5f44fb67a81d5b958f6350e';
const SNAPSHOT_URL = '/ui-assets/creator-workbench/goldmaster-snapshots.json.gz';
const FIDELITY_CSS_URL = '/ui-assets/creator-workbench/goldmaster-fidelity.css.gz';

async function readGzipResponse(url: string): Promise<ReadableStream<Uint8Array>> {
  const response = await fetch(url, { cache: 'no-store', credentials: 'same-origin' });
  if (!response.ok || !response.body) throw new Error('预览资源加载失败：' + url + ' (' + response.status + ')');
  if (typeof DecompressionStream === 'undefined') throw new Error('当前浏览器不支持隔离预览资源解压。');
  return response.body.pipeThrough(new DecompressionStream('gzip'));
}

async function loadSnapshotBundle(): Promise<GoldMasterBundle> {
  const stream = await readGzipResponse(SNAPSHOT_URL);
  const bundle = await new Response(stream).json() as GoldMasterBundle;
  if (bundle.referenceSha256 !== GOLDMASTER_REFERENCE_SHA) throw new Error('Gold Master 基准指纹不匹配，拒绝加载预览。');
  return bundle;
}

async function loadFidelityCss(): Promise<string> {
  const stream = await readGzipResponse(FIDELITY_CSS_URL);
  return new Response(stream).text();
}

export function CreatorWorkbenchPreviewPage() {
  const [bundle, setBundle] = useState<GoldMasterBundle | null>(null);
  const [mode, setMode] = useState<GoldMasterMode>('writing');
  const [loadError, setLoadError] = useState('');
  const previewRef = useRef<CreatorWorkbenchGoldMasterHandle | null>(null);

  useEffect(() => {
    let cancelled = false;
    let styleElement: HTMLStyleElement | null = null;
    Promise.all([loadSnapshotBundle(), loadFidelityCss()])
      .then(([loadedBundle, css]) => {
        if (cancelled) return;
        styleElement = document.createElement('style');
        styleElement.id = 'creator-workbench-goldmaster-fidelity';
        styleElement.dataset.source = GOLDMASTER_REFERENCE_SHA;
        styleElement.textContent = css;
        document.head.appendChild(styleElement);
        setBundle(loadedBundle);
      })
      .catch(error => {
        if (!cancelled) setLoadError(error instanceof Error ? error.message : '预览资源加载失败。');
      });
    return () => {
      cancelled = true;
      styleElement?.remove();
    };
  }, []);

  const previewSidebar = <>
    <div className="v7-nav-label">运营</div>
    <button className="v7-side-nav" type="button" aria-current="page" onClick={() => undefined}>
      <NavIcon name="publications"/><span>创作</span><small>5</small>
    </button>
    {([['publications', '发布记录', '1'], ['daily', '每日数据', ''], ['accounts', '账号管理', '']] as const).map(([icon, label, count]) => (
      <button key={label} className="v7-side-nav" type="button" onClick={() => undefined}>
        <NavIcon name={icon}/><span>{label}</span>{count ? <small>{count}</small> : null}
      </button>
    ))}
    <div className="v7-side-foot">运营工作区：创作、发布记录、每日数据与账号管理在同一业务入口内完成。</div>
  </>;

  const meta = mode === 'desk' ? <>
    <span className={styles.metaCount}>选题组织 · 材料编排 · 多账号开稿</span>
    <ActionButton variant="primary" onClick={event => previewRef.current?.dispatchAction('deskExit', event.currentTarget)}>返回写稿</ActionButton>
  </> : <>
    <span className={styles.metaCount}>今日 <b>6</b> 篇 · 待完成 <b>5</b> · 已发布 <b>1</b></span>
    <ActionButton variant="outline" onClick={event => previewRef.current?.dispatchAction('openAssets', event.currentTarget)}>资产库</ActionButton>
    <ActionButton variant="quiet" onClick={event => previewRef.current?.dispatchAction('path', event.currentTarget)}>使用路径</ActionButton>
    <ActionButton variant="quiet" onClick={event => previewRef.current?.dispatchAction('tour', event.currentTarget)}>体验清单 0/8</ActionButton>
  </>;

  return (
    <OpsFrame section="creation" domain="preview" scope={<span>创作演示</span>} tabs={null} meta={meta}
      preview previewScope={mode === 'desk' ? '选题与材料' : '创作'} previewDeskMode={mode === 'desk'} previewSidebar={previewSidebar}>
      <main className={styles.main} aria-label="创作工作台">
        {loadError ? <div className={styles.error} role="alert">{loadError}</div> : null}
        {!bundle && !loadError ? <div className={styles.loading} role="status">正在载入 Gold Master 隔离预览…</div> : null}
        {bundle ? <CreatorWorkbenchGoldMasterAdapter ref={previewRef} snapshots={bundle} onModeChange={setMode}/> : null}
      </main>
    </OpsFrame>
  );
}

export default CreatorWorkbenchPreviewPage;
