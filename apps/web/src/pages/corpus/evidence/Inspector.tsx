import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react';
import { DossierPanel } from './DossierPanel';
import { EvidencePanel } from './EvidencePanel';
import { Lightbox } from './Lightbox';
import { MaterialsPanel } from './MaterialsPanel';
import { SourceNote } from './Primitives';
import { ReadingPanel, type PanelContext } from './ReadingPanel';
import { TracePanel } from './TracePanel';
import { errorCode } from './evidenceFeedback';
import { apiPath } from './evidencePaths';
import { workTitle } from './evidencePresent';
import { PANELS, TABS, type PanelWidth, type TabKey } from './evidenceQuery';
import type { WorkDetail, WorkItem } from './evidenceTypes';
import { mediaObjects, singleViewerObject, type ViewerObject } from './mediaObjects';
import { reobservationAccess } from './reobservationAccess';
import { useCommentChannel } from './useCommentChannel';
import { useReobservation } from './useReobservation';
import page from './EvidencePage.module.css';
import styles from './Inspector.module.css';

export interface DetailState {
  status: 'idle' | 'loading' | 'error' | 'ready' | 'incomplete';
  data: WorkDetail | null;
  error: unknown;
  /** 作品是按地址里的 `work` 直读的（不在已读到的页内）。 */
  direct: boolean;
}

export interface InspectorProps {
  domain: string;
  selectedRef: string | null;
  listItem: WorkItem | null;
  detail: DetailState;
  tab: TabKey;
  width: PanelWidth;
  /** 抽屉模式（≤1180px）：检查器是盖在列表上的抽屉，宽度控件与「×」隐藏。 */
  drawer: boolean;
  open: boolean;
  onTab: (tab: TabKey) => void;
  onWidth: (width: PanelWidth) => void;
  onClose: () => void;
  onBack: () => void;
  onRetry: () => void;
  /** 重新读取作品详情；返回是否成功（复观测终态后用来对账）。 */
  onRefresh: () => Promise<boolean>;
  /** 重新分层成功：列表与详情一并重读。 */
  onApplied: () => void;
}

const WIDTH_LABELS: Record<PanelWidth, string> = { normal: '标准', wide: '加宽', focus: '专注' };
const WIDTH_TITLES: Record<PanelWidth, string> = { normal: '标准宽度', wide: '加宽', focus: '专注' };

function stateText(props: Pick<InspectorProps, 'selectedRef' | 'detail'>): { text: string; code: string | null } | null {
  const { selectedRef, detail } = props;
  if (selectedRef === null) return { text: '尚未选择作品', code: null };
  if (detail.status === 'loading' || detail.status === 'idle') return { text: '正在读取当前作品详情', code: null };
  if (detail.status === 'incomplete') return { text: '来源信息不完整', code: null };
  if (detail.status === 'error') {
    const code = errorCode(detail.error);
    return code === 'material_not_found' && detail.direct ? { text: '当前领域无法读取这篇作品', code } : { text: '当前作品详情读取失败', code };
  }
  return null;
}

function unavailableNote(detail: DetailState): string | null {
  if (detail.status === 'incomplete') return '来源信息不完整：列表没有提供可用的同源 detailUrl。';
  if (detail.status !== 'error') return null;
  return errorCode(detail.error) === 'material_not_found' && detail.direct
    ? '这篇作品可能未收录于所选领域，或当前缺少可读取的材料；请切换领域或从作品列表重新选择。'
    : '当前详情未读取，列表通道仍可查看，但不能据此补造检查器内容。';
}

interface BodyProps {
  detail: WorkDetail;
  listItem: WorkItem | null;
  domain: string;
  tab: TabKey;
  onRefresh: () => Promise<boolean>;
  onApplied: () => void;
}

/** 一件作品的面板内容。按作品引用重建：评论原文、复观测结果、底稿状态都不跨作品保留。 */
function WorkBody({ detail, listItem, domain, tab, onRefresh, onApplied }: BodyProps) {
  const { item, inspector, channels } = detail;
  const commentsUrl = apiPath(channels.commentsUrl);
  const comments = useCommentChannel(commentsUrl);
  const reobservation = useReobservation({ actionPath: reobservationAccess(item, channels.reobservation).actionPath, onTerminal: onRefresh });
  const [viewer, setViewer] = useState<{ objects: readonly ViewerObject[]; index: number } | null>(null);
  const fragment = (listItem ?? item).fragment;
  const context: PanelContext = {
    item,
    inspector,
    channels: { ...channels, commentsUrl },
    listItem,
    comments,
    onOpenViewer: index => setViewer({ objects: mediaObjects(inspector, fragment), index }),
  };
  const recheck = () => void onRefresh();
  return (
    <>
      <Panel id="overview" active={tab === 'overview'}>
        <ReadingPanel context={context} domain={domain} reobservation={reobservation} onRecheck={recheck} />
      </Panel>
      <Panel id="evidence" active={tab === 'evidence'}>
        <EvidencePanel context={context} />
      </Panel>
      <Panel id="dossier" active={tab === 'dossier'}>
        <DossierPanel
          work={item.ref}
          domain={domain}
          active={tab === 'dossier'}
          platformComments={item.display.engagement.comment}
          onOpenImage={media => setViewer({ objects: [singleViewerObject(media)], index: 0 })}
          onApplied={onApplied}
        />
      </Panel>
      <Panel id="materials" active={tab === 'materials'}>
        <MaterialsPanel context={context} />
      </Panel>
      <Panel id="trace" active={tab === 'trace'}>
        <TracePanel context={context} />
      </Panel>
      {viewer ? <Lightbox objects={viewer.objects} index={viewer.index} onIndex={index => setViewer({ ...viewer, index })} onClose={() => setViewer(null)} /> : null}
    </>
  );
}

function Panel({ id, active, children }: { id: TabKey; active: boolean; children: React.ReactNode }) {
  return (
    <div role="tabpanel" id={`evidence-panel-${id}`} aria-labelledby={`evidence-tab-${id}`} hidden={!active}>
      {children}
    </div>
  );
}

/**
 * 检查器：头部（宽度、关闭、状态、五个标签）与五个面板。标签 `role=tab`，漫游焦点，← → Home End 切换；
 * 抽屉模式下 Esc 关闭、「← 返回当前作品」把所选行滚入视野。关闭着的抽屉是惰性的（不在 tab 顺序里、读屏读不到）。
 */
export function Inspector(props: InspectorProps) {
  const { domain, selectedRef, listItem, detail, tab, width, drawer, open, onTab, onWidth, onClose, onBack, onRetry, onRefresh, onApplied } = props;
  const titleId = useId();
  const tabs = useRef<Record<string, HTMLButtonElement | null>>({});
  const state = stateText(props);
  const note = unavailableNote(detail);
  const title = detail.data ? workTitle(detail.data.item) : listItem ? workTitle(listItem) : '请选择一个作品材料集合';

  // 抽屉打开时 Esc 逐层关闭：对话框与浮层先处理（它们会 preventDefault），这里只在没人处理时关抽屉。
  useEffect(() => {
    if (!drawer || !open) return;
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key !== 'Escape' || event.defaultPrevented) return;
      if (document.querySelector('dialog[open]')) return;
      event.preventDefault();
      onClose();
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [drawer, open, onClose]);

  function onTabKey(event: KeyboardEvent) {
    const keys = ['ArrowLeft', 'ArrowRight', 'Home', 'End'];
    if (!keys.includes(event.key)) return;
    event.preventDefault();
    const index = TABS.findIndex(([key]) => key === tab);
    let next = index;
    if (event.key === 'ArrowLeft') next = (index - 1 + TABS.length) % TABS.length;
    if (event.key === 'ArrowRight') next = (index + 1) % TABS.length;
    if (event.key === 'Home') next = 0;
    if (event.key === 'End') next = TABS.length - 1;
    const target = TABS[next]?.[0];
    if (target) {
      onTab(target);
      tabs.current[target]?.focus();
    }
  }

  return (
    <aside className={styles.inspector} aria-labelledby={titleId} data-width={width} data-drawer-open={drawer && open} inert={drawer && !open ? true : undefined}>
      <header className={styles.head}>
        <button type="button" className={styles.back} onClick={onBack}>
          ← 返回当前作品
        </button>
        <h2 id={titleId} className="v7-sr-only">
          {title}
        </h2>
        {drawer ? null : (
          <div className={styles.tools}>
            <div className={styles.widths} role="group" aria-label="检查器宽度">
              {PANELS.map(candidate => (
                <button key={candidate} type="button" aria-pressed={width === candidate} title={WIDTH_TITLES[candidate]} onClick={() => onWidth(candidate)}>
                  <i className={styles.glyph} data-width={candidate} aria-hidden="true" />
                  <span className="v7-sr-only">{WIDTH_LABELS[candidate]}</span>
                </button>
              ))}
            </div>
            <button type="button" className={styles.close} title="关闭检查器" onClick={onClose}>
              <span aria-hidden="true">×</span>
              <span className="v7-sr-only">关闭检查器</span>
            </button>
          </div>
        )}
        <div className={styles.state} role="status" data-empty={state === null}>
          {state ? (
            <>
              <strong>{state.text}</strong>
              {state.code ? <code>{state.code}</code> : null}
              {detail.status === 'error' ? (
                <button type="button" className={page.textButton} onClick={onRetry}>
                  重试读取
                </button>
              ) : null}
            </>
          ) : null}
        </div>
        <div className={styles.tabs} role="tablist" aria-label="作品证据详情" onKeyDown={onTabKey}>
          {TABS.map(([key, label]) => (
            <button
              key={key}
              ref={element => {
                tabs.current[key] = element;
              }}
              type="button"
              role="tab"
              id={`evidence-tab-${key}`}
              aria-controls={`evidence-panel-${key}`}
              aria-selected={tab === key}
              tabIndex={tab === key ? 0 : -1}
              onClick={() => onTab(key)}
            >
              {label}
            </button>
          ))}
        </div>
      </header>
      <div className={styles.body}>
        {detail.status === 'ready' && detail.data && selectedRef ? (
          <WorkBody key={selectedRef} detail={detail.data} listItem={listItem} domain={domain} tab={tab} onRefresh={onRefresh} onApplied={onApplied} />
        ) : (
          TABS.map(([key]) => (
            <Panel key={key} id={key} active={tab === key}>
              {tab === key && note ? <SourceNote>{note}</SourceNote> : null}
            </Panel>
          ))
        )}
      </div>
    </aside>
  );
}
