import { useEffect, useId, useRef, useState, type KeyboardEvent } from 'react';
import { CommentDeepCollect } from './CommentDeepCollect';
import { DossierPanel } from './DossierPanel';
import { EvidencePanel } from './EvidencePanel';
import { Lightbox } from './Lightbox';
import { TechnicalFacts } from './TechnicalFacts';
import { ReobservationSection } from './ReobservationSection';
import { AuthorFact, Engagement, MaterialLegend, MaterialRail, QuoteBlock } from './WorkParts';
import { Section } from './Primitives';
import { materialSummary, publishedCopy } from './evidencePresent';
import { MaterialsPanel } from './MaterialsPanel';
import { ObservationHistory } from './ObservationHistory';
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
        <ReadingPanel context={context} domain={domain} />
      </Panel>
      <Panel id="dossier" active={tab === 'dossier'}>
        {fragment ? <Section title="当前列表引用的原声"><QuoteBlock fragment={fragment} wrap /></Section> : null}
        <DossierPanel
          work={item.ref}
          domain={domain}
          active={tab === 'dossier'}
          platformComments={item.display.engagement.comment}
          onOpenImage={media => setViewer({ objects: [singleViewerObject(media)], index: 0 })}
          onApplied={onApplied}
        />
        <details className={styles.technical}>
          <summary>原始识别、评论通道与来源回执（高级核对）</summary>
          <EvidencePanel context={context} />
        </details>
      </Panel>
      {/* 采集记录（Mog 2026-10-23）：先答「看过几次、数字怎么变、要不要再看、评论要不要采全」；
          材料完整度、媒体台账、来源编号与技术事实一项不删，统一收进默认折叠的「技术信息」。 */}
      <Panel id="trace" active={tab === 'trace'}>
        <ObservationHistory item={item} inspector={inspector} />
        <ReobservationSection item={item} channel={channels.reobservation} domain={domain} state={reobservation} onRecheck={recheck} />
        <CommentDeepCollect item={item} inspector={inspector} domain={domain} />
        <details className={styles.technical}>
          <summary>技术信息（排查用）</summary>
          <Section title="材料完整度">
            <MaterialRail summary={materialSummary(item)} />
            <MaterialLegend summary={materialSummary(item)} />
          </Section>
          <MaterialsPanel context={context} />
          <TracePanel context={context} />
          <TechnicalFacts item={item} inspector={inspector} reobservation={channels.reobservation} />
        </details>
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
 * 检查器：头部（宽度、关闭、状态、三个标签）与三个面板。标签 `role=tab`，漫游焦点，← → Home End 切换；
 * 抽屉模式下 Esc 关闭、「← 返回当前作品」把所选行滚入视野。关闭着的抽屉是惰性的（不在 tab 顺序里、读屏读不到）。
 */
export function Inspector(props: InspectorProps) {
  const { domain, selectedRef, listItem, detail, tab, width, drawer, open, onTab, onWidth, onClose, onBack, onRetry, onRefresh, onApplied } = props;
  const titleId = useId();
  const surface = useRef<HTMLElement>(null);
  const back = useRef<HTMLButtonElement>(null);
  const tabs = useRef<Record<string, HTMLButtonElement | null>>({});
  const state = stateText(props);
  const note = unavailableNote(detail);
  const work = detail.data?.item ?? listItem;
  const title = work ? workTitle(work) : '请选择一个作品材料集合';

  useEffect(() => {
    if (drawer && open) back.current?.focus({ preventScroll: true });
  }, [drawer, open]);

  // S1只读非模态抽屉：内层浮层优先；拖出抽屉不算点空白，列表仍可直接切换作品。
  useEffect(() => {
    if (!drawer || !open) return;
    let outside = false;
    const blocked = () => document.querySelector('dialog[open], [role="listbox"]') !== null;
    const isOutside = (event: PointerEvent) => event.target instanceof Node && !surface.current?.contains(event.target);
    const down = (event: PointerEvent) => { outside = !blocked() && isOutside(event); };
    const up = (event: PointerEvent) => {
      if (outside && !blocked() && isOutside(event)) onClose();
      outside = false;
    };
    const cancel = () => { outside = false; };
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key !== 'Escape' || event.defaultPrevented || blocked()) return;
      event.preventDefault(); onClose();
    };
    document.addEventListener('keydown', onKey);
    document.addEventListener('pointerdown', down, true);
    document.addEventListener('pointerup', up, true);
    document.addEventListener('pointercancel', cancel, true);
    return () => {
      document.removeEventListener('keydown', onKey);
      document.removeEventListener('pointerdown', down, true);
      document.removeEventListener('pointerup', up, true);
      document.removeEventListener('pointercancel', cancel, true);
    };
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
    <aside ref={surface} className={styles.inspector} aria-labelledby={titleId} data-width={width} data-drawer-open={drawer && open} inert={drawer && !open ? true : undefined}>
      <header className={styles.head}>
        <button ref={back} type="button" className={styles.back} onClick={onBack}>
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
        {work ? <div className={styles.workSummary}>
          <h3 className={styles.workTitle} title={title}>{title}</h3>
          <div className={styles.workByline}>
            <AuthorFact item={work} />
            <span className={styles.workPlatform}>{work.platform.toUpperCase()}</span>
            <span className={styles.workDate}>{publishedCopy(work.display, { compact: true, withFieldName: false })}</span>
          </div>
          <Engagement engagement={work.display.engagement} />
        </div> : null}
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
      <div className={styles.body} data-current-tab={tab}>
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
