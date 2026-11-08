import { useEffect, useRef, useState, type CSSProperties, type MouseEvent as ReactMouseEvent, type PointerEvent, type WheelEvent } from 'react';
import { OpsFrame } from '../ops/OpsFrame';
import { NavIcon } from '../../shared/Sidebar';
import { ActionButton } from '../../shared/ActionButton';
import { EmptyState } from '../../shared/EmptyState';
import { FindField } from '../../shared/FindField';
import { InlineNotice } from '../../shared/InlineNotice';
import { SelectField } from '../../shared/SelectField';
import styles from './CreatorWorkbenchPreviewPage.module.css';
import { INITIAL_DRAFTS, type PreviewDraft as Draft, type PreviewParagraph as Paragraph } from './CreatorWorkbenchPreviewFixtures';
import { CreatorWorkbenchPreviewG5States, CreatorWorkbenchPreviewTour, type PreviewDialogKind } from './CreatorWorkbenchPreviewG5States';

type InspectorTab = 'checks' | 'materials' | 'titles' | 'preview';
type AssetKind = 'persona' | 'experience' | 'product' | 'bridge' | 'sample' | 'style';
type DeskItem = {
  id: string; kind: 'frame' | 'material' | 'note'; title: string; body: string;
  x: number; y: number; width: number; height: number;
};

const ASSETS: { id: AssetKind; label: string; description: string }[] = [
  { id: 'persona', label: '人设事实', description: '账号身份、可确认事实与使用状态' },
  { id: 'experience', label: '经历库', description: '本人确认的经历、来源与使用记录' },
  { id: 'product', label: '产品事实卡', description: '产品信息、适用边界与必带提示' },
  { id: 'bridge', label: '衔接句', description: '按账号关系管理的衔接表达' },
  { id: 'sample', label: '样板库', description: '结构步骤、审核状态与版本' },
  { id: 'style', label: '风格', description: '结构、节奏、用词与不采用的表达' },
];

const INITIAL_DESK_ITEMS: DeskItem[] = [
  { id: 'f1', kind: 'frame', title: '选题框 01', body: '把一个复杂问题拆成可以马上开始的小步骤。', x: 90, y: 90, width: 300, height: 188 },
  { id: 'f2', kind: 'frame', title: '选题框 02', body: '复盘一次沟通，找到更合适的表达顺序。', x: 570, y: 340, width: 300, height: 188 },
  { id: 'f3', kind: 'frame', title: '选题框 03', body: '从日常细节中提取一个可验证的观察。', x: 1060, y: 100, width: 300, height: 188 },
  { id: 'm1', kind: 'material', title: '材料摘录 01', body: '真正有效的改变，往往来自一次次小的调整。', x: 440, y: 90, width: 250, height: 150 },
  { id: 'm2', kind: 'material', title: '材料摘录 02', body: '先把眼前最重要的一步做好，再决定下一步。', x: 930, y: 360, width: 250, height: 150 },
  { id: 'n1', kind: 'note', title: '自己的想法', body: '可以从一个具体场景切入。', x: 150, y: 390, width: 230, height: 132 },
  { id: 'n2', kind: 'note', title: '待补充', body: '这里还需要一个真实例子。', x: 1350, y: 380, width: 230, height: 132 },
];

const ASSET_ROWS: Record<AssetKind, { title: string; meta: string; body: string }[]> = {
  persona: [
    { title: '表达偏好', meta: '人工记录 · 已确认', body: '偏好具体场景，避免抽象结论。' },
    { title: '常用表达', meta: '人工记录 · 待确认', body: '用自己的话描述过程，不替别人下结论。' },
    { title: '账号简介', meta: '账号身份 · 已核对', body: '简介信息需要由账号负责人确认。' },
  ],
  experience: [
    { title: '一次临时调整计划的经历', meta: '本人确认 · 可用于草稿', body: '记录一个具体时间、场景和当时采取的行动。' },
    { title: '第一次尝试新方法', meta: '待本人确认', body: '内容尚未确认，不会进入可用材料。' },
  ],
  product: [
    { title: '产品基础事实', meta: '机构确认 · 第 2 版', body: '名称、适用对象、服务范围和使用边界。' },
  ],
  bridge: [
    { title: '家长身份衔接句', meta: '已审核 · 使用 4 次', body: '先说明自己的关系，再自然进入本篇主题。' },
    { title: '老师身份衔接句', meta: '待审核 · 使用 0 次', body: '先交代观察场景，不暗示结果承诺。' },
  ],
  sample: [
    { title: '场景—变化—复盘', meta: 'YB-0001 · 已审核', body: '开头场景 / 具体变化 / 原因复盘 / 下一步行动。' },
    { title: '问题—尝试—反馈', meta: 'YB-0002 · 待审核', body: '具体问题 / 尝试过程 / 可观察反馈 / 个人判断。' },
  ],
  style: [
    { title: '具体场景表达', meta: '合成示例 · 非正式资产', body: '结构：先场景后观点；节奏：短句与长句交替；用词：具体、克制。' },
  ],
};

const FILTER_OPTIONS = [
  { value: 'all', label: '全部材料' },
  { value: 'material', label: '材料摘录' },
  { value: 'idea', label: '自己的想法' },
] as const;

export function CreatorWorkbenchPreviewPage() {
  const [mode, setMode] = useState<'writing' | 'desk'>('writing');
  const [drafts, setDrafts] = useState(INITIAL_DRAFTS);
  const [draftId, setDraftId] = useState('d1');
  const [inspectorTab, setInspectorTab] = useState<InspectorTab>('checks');
  const [assetOpen, setAssetOpen] = useState(false);
  const [assetKind, setAssetKind] = useState<AssetKind>('persona');
  const [assetSelection, setAssetSelection] = useState(0);
  const [assetSearch, setAssetSearch] = useState('');
  const [notice, setNotice] = useState('');
  const [g5Dialog, setG5Dialog] = useState<PreviewDialogKind>(null);
  const [tourOpen, setTourOpen] = useState(false);
  const [productEditing, setProductEditing] = useState(false);
  const [productDraft, setProductDraft] = useState('演示产品事实卡');
  const [coverCompare, setCoverCompare] = useState(false);
  const [experienceTargetId, setExperienceTargetId] = useState<string | null>(null);
  const [deskSearch, setDeskSearch] = useState('');
  const [deskSearchOpen, setDeskSearchOpen] = useState(true);
  const [deskInspectorOpen, setDeskInspectorOpen] = useState(true);
  const [deskFilter, setDeskFilter] = useState<(typeof FILTER_OPTIONS)[number]['value']>('all');
  const [deskItems, setDeskItems] = useState(INITIAL_DESK_ITEMS);
  const [selectedDeskId, setSelectedDeskId] = useState<string | null>('f1');
  const [focusedFrame, setFocusedFrame] = useState<string | null>(null);
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 30, y: 24 });
  const [queueDrawer, setQueueDrawer] = useState(false);
  const [inspectorDrawer, setInspectorDrawer] = useState(false);
  const assetTrigger = useRef<HTMLButtonElement>(null);
  const g5Trigger = useRef<HTMLButtonElement | null>(null);
  const g5WasOpened = useRef(false);
  const assetWasOpened = useRef(false);
  const nextLocalDeskId = useRef(0);
  const nextLocalExperienceId = useRef(0);
  const dialogRef = useRef<HTMLDialogElement>(null);
  const pointerRef = useRef<{ x: number; y: number; panX: number; panY: number } | null>(null);
  const currentDraft = drafts.find(item => item.id === draftId) ?? drafts[0]!;

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (assetOpen && !dialog.open) dialog.showModal();
    if (!assetOpen && dialog.open) dialog.close();
  }, [assetOpen]);

  useEffect(() => {
    if (assetOpen) {
      assetWasOpened.current = true;
    } else if (assetWasOpened.current) {
      assetWasOpened.current = false;
      assetTrigger.current?.focus({ preventScroll: true });
    }
  }, [assetOpen]);

  useEffect(() => {
    if (g5Dialog) g5WasOpened.current = true;
    else if (g5WasOpened.current) {
      g5WasOpened.current = false;
      g5Trigger.current?.focus({ preventScroll: true });
    }
  }, [g5Dialog]);

  useEffect(() => {
    if (mode !== 'desk') return;
    const onKey = (event: KeyboardEvent) => {
      if (event.target instanceof HTMLElement && event.target.closest('input,textarea,select,button,[contenteditable=true]')) return;
      if (event.key === 'Escape') { setFocusedFrame(null); return; }
      if (/^F[1-4]$/.test(event.key)) { setFocusedFrame('f' + event.key.slice(1)); setSelectedDeskId('f' + event.key.slice(1)); event.preventDefault(); }
      if (event.key === 'ArrowUp' || event.key === 'ArrowDown' || event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
        const delta = event.shiftKey ? 50 : 10;
        const dx = event.key === 'ArrowLeft' ? -delta : event.key === 'ArrowRight' ? delta : 0;
        const dy = event.key === 'ArrowUp' ? -delta : event.key === 'ArrowDown' ? delta : 0;
        setDeskItems(items => items.map(item => item.id === selectedDeskId ? { ...item, x: item.x + dx, y: item.y + dy } : item));
        event.preventDefault();
      }
      if (event.key === 'Delete' && selectedDeskId) {
        setDeskItems(items => items.filter(item => item.id !== selectedDeskId));
        setSelectedDeskId(null);
        event.preventDefault();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [mode, selectedDeskId]);

  const updateDraft = (patch: Partial<Draft>) => setDrafts(items => items.map(item => item.id === draftId ? { ...item, ...patch } : item));
  const updateParagraph = (id: string, text: string) => updateDraft({ paragraphs: currentDraft.paragraphs.map(item => item.id === id ? { ...item, text } : item) });
  const previewOnly = (action: string) => setNotice(action + '：当前为隔离预览，未调用正式 API、模型、采集或发布服务。');
  const openG5 = (kind: Exclude<PreviewDialogKind, null>, trigger: HTMLButtonElement) => {
    g5Trigger.current = trigger;
    setG5Dialog(kind);
  };
  const useSyntheticExperience = (body: string) => {
    const index = currentDraft.paragraphs.findIndex(item => item.id === experienceTargetId);
    const insertAt = index < 0 ? currentDraft.paragraphs.length : index + 1;
    const newParagraph: Paragraph = { id: currentDraft.id + '-synthetic-experience-' + (++nextLocalExperienceId.current), role: '经历', text: body };
    updateDraft({ paragraphs: [
      ...currentDraft.paragraphs.slice(0, insertAt), newParagraph, ...currentDraft.paragraphs.slice(insertAt),
    ] });
    previewOnly('本地插入已确认的合成经历');
  };
  const openDesk = () => { setMode('desk'); setFocusedFrame(null); setZoom(1); setPan({ x: 30, y: 24 }); setDeskSearchOpen(true); setDeskInspectorOpen(true); };
  const addDeskItem = (kind: DeskItem['kind']) => {
    const id = 'local-' + (++nextLocalDeskId.current);
    const item: DeskItem = { id, kind, title: kind === 'frame' ? '新选题框' : '新便签', body: '', x: 180 + deskItems.length * 24, y: 180 + deskItems.length * 18, width: kind === 'frame' ? 300 : 230, height: kind === 'frame' ? 188 : 132 };
    setDeskItems(items => [...items, item]);
    setSelectedDeskId(id);
    previewOnly(kind === 'frame' ? '已在本地画布添加选题框' : '已在本地画布添加便签');
  };

  const onCanvasPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || (event.target as HTMLElement).closest('button,input,textarea,[data-canvas-item]')) return;
    pointerRef.current = { x: event.clientX, y: event.clientY, panX: pan.x, panY: pan.y };
    event.currentTarget.setPointerCapture(event.pointerId);
  };
  const onCanvasPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const start = pointerRef.current;
    if (!start) return;
    setPan({ x: start.panX + event.clientX - start.x, y: start.panY + event.clientY - start.y });
  };
  const onCanvasPointerUp = () => { pointerRef.current = null; };
  const onCanvasWheel = (event: WheelEvent<HTMLDivElement>) => {
    if (!event.ctrlKey && !event.metaKey) return;
    event.preventDefault();
    setZoom(value => Math.max(0.45, Math.min(2.4, value * (event.deltaY < 0 ? 1.08 : 0.92))));
  };
  const onCanvasDoubleClick = (event: ReactMouseEvent<HTMLDivElement>) => {
    if ((event.target as HTMLElement).closest('button,[data-canvas-item]')) return;
    addDeskItem('note');
  };
  const deskVisibleItems = deskItems.filter(item => {
    const matchesText = !deskSearch || (item.title + ' ' + item.body).toLowerCase().includes(deskSearch.toLowerCase());
    const matchesType = deskFilter === 'all' || (deskFilter === 'idea' ? item.kind === 'note' : item.kind === 'material');
    return matchesText && matchesType;
  });
  const selectedDeskItem = deskItems.find(item => item.id === selectedDeskId) ?? null;
  const assetItems = ASSET_ROWS[assetKind].filter(item => !assetSearch || (item.title + item.body).includes(assetSearch));
  const selectedAssetItem = assetItems[Math.min(assetSelection, assetItems.length - 1)] ?? null;

  return (
    <div className={styles.root} data-preview-only="true">
      <OpsFrame section="creation" domain="preview" scope={<span>创作演示</span>} immersive={mode === 'desk'} preview
        previewSidebar={<>
          <div className="v7-nav-label">运营</div>
          <button className="v7-side-nav" type="button" aria-current="page" onClick={() => previewOnly('创作工作台')}>
            <NavIcon name="publications"/><span>创作</span><small>5</small>
          </button>
          {([['publications','发布记录','1'],['daily','每日数据',''],['accounts','账号管理','']] as const).map(([icon,label,count]) => (
            <button key={label} className="v7-side-nav" type="button" onClick={() => previewOnly(label)}>
              <NavIcon name={icon}/><span>{label}</span>{count ? <small>{count}</small> : null}
            </button>
          ))}
          <div className="v7-side-foot">运营工作区：创作、发布记录、每日数据与账号管理在同一业务入口内完成。</div>
        </>}
        tabs={<>
          <button type="button" aria-current={mode === 'writing' ? 'page' : undefined} onClick={() => setMode('writing')}>写稿</button>
          <button type="button" aria-current={mode === 'desk' ? 'page' : undefined} onClick={openDesk}>材料桌</button>
        </>}
        meta={<>
          <span className={styles.metaCount}>今日 <b>6</b> 篇 · 待完成 <b>5</b> · 已发布 <b>1</b></span>
          <ActionButton ref={assetTrigger} variant="outline" onClick={() => { setAssetOpen(true); setAssetKind('persona'); setAssetSelection(0); }}>资产库</ActionButton>
          <ActionButton variant="quiet" onClick={event => openG5('path-guide', event.currentTarget)}>使用路径</ActionButton>
          <ActionButton variant="quiet" onClick={() => setTourOpen(open => !open)} aria-expanded={tourOpen}>体验清单 0/8</ActionButton>
        </>}>
        <main className={styles.main} aria-label="创作工作台">
          <section className={[styles.write, queueDrawer ? styles.queueOpen : '', inspectorDrawer ? styles.inspectorOpen : ''].filter(Boolean).join(' ')} hidden={mode !== 'writing'}>
            <aside className={styles.queue} aria-label="稿件队列">
              <div className={styles.queueHead}>
                <div className={styles.queueHeadTop}>
                  <h2>今日 · 10月27日 周一</h2>
                  <button type="button" className={styles.smallAction} onClick={() => {
                    const id = 'new-' + (drafts.length + 1);
                    setDrafts(items => [...items, { ...INITIAL_DRAFTS[0]!, id, title: '新建稿件' }]);
                    setDraftId(id);
                    previewOnly('已建立本地演示稿件');
                  }}>＋ 新选题</button>
                </div>
                <ActionButton className={styles.queueDeskButton} variant="quiet" onClick={openDesk}>
                  <span className={styles.deskEntryIcon} aria-hidden="true">▣</span>选题与材料<span className={styles.dataCount}>4 个选题</span>
                </ActionButton>
                <button type="button" className={styles.mobileRail} aria-label="关闭稿件队列" onClick={() => setQueueDrawer(false)}>关闭</button>
              </div>
              <div className={styles.queueScroll}>
                <div className={styles.groupTitle}>选题中 · 还没开稿 <b>1</b></div>
                <button type="button" className={styles.frameItem} onClick={openDesk}>
                  <strong>让孩子自己整理书包</strong>
                  <small>XT-1027-01 · 示例妈妈 · 6 张材料</small>
                  <span><i />在选题与材料整理</span>
                </button>
                <div className={styles.groupTitle}>稿件 <b>{drafts.length}</b></div>
                {drafts.map(item => (
                  <button key={item.id} type="button" className={styles.draftItem}
                    aria-current={item.id === draftId ? 'true' : undefined}
                    onClick={() => { setDraftId(item.id); setInspectorTab('checks'); setQueueDrawer(false); }}>
                    <strong>{item.title}</strong>
                    <small>{item.account}　{item.kind}　{item.origin}</small>
                    <span><i data-level={item.must ? 'must' : item.warn ? 'warn' : 'ok'} />
                      {item.must ? '必改 ' + item.must : item.state}{item.warn ? ' · 提醒 ' + item.warn : ''}
                    </span>
                  </button>
                ))}
              </div>
              <div className={styles.queueFoot}><i />运营工作区：创作、发布记录、每日数据与账号管理在同一业务入口内完成。</div>
            </aside>

            <section className={styles.editor} aria-label="稿件编辑区">
              <header className={styles.editorHead} data-testid="editor-head">
                <button type="button" className={styles.mobileRail} aria-label="打开今日稿件队列"
                  aria-expanded={queueDrawer} onClick={() => setQueueDrawer(value => !value)}>今日 {drafts.length} 篇</button>
                <div className={styles.editorIdentity}>
                  <span className={styles.avatar}>{currentDraft.accountMark}</span>
                  <strong>{currentDraft.account}</strong>
                  <span className={styles.identityChip}>身份待确认</span>
                </div>
                <button type="button" className={styles.frameChip} onClick={() => previewOnly('补充选题框')}>＋ 补一个选题框</button>
                <div className={styles.editorMeta}>
                  <span>{currentDraft.kind} · {currentDraft.origin}</span>
                  <span>样板 <b>{currentDraft.sampleCode}</b> · {currentDraft.sampleName}</span>
                  <span>风格 · {currentDraft.styleName}</span>
                </div>
                <button type="button" className={styles.mobileRail} aria-label="打开稿件检查器"
                  aria-expanded={inspectorDrawer} onClick={() => setInspectorDrawer(value => !value)}>检查 · 必改 {currentDraft.must}</button>
              </header>
              <div className={styles.editorScroll} data-testid="editor-scroll">
                <div className={styles.editorInner} data-testid="editor-inner">
                  <div className={styles.intentRow} data-testid="intent-row"><span>选题意图</span><b>{currentDraft.angle}</b></div>
                  {currentDraft.related ? (
                    <div className={styles.related} data-testid="related-card">
                      和我方爆款《{currentDraft.related.title}》（<b>{currentDraft.related.reads.toLocaleString('zh-CN')}</b> 阅读）意思接近，要算成同一个选题吗？
                      <button type="button" onClick={() => previewOnly('已在本地预览中关联为同一选题')}>关联成同一选题</button>
                      <button type="button" onClick={() => updateDraft({ related: undefined })}>不关联，按试新算</button>
                    </div>
                  ) : null}
                  <div className={styles.coverRow}>
                    <div className={styles.coverColumn}>
                      <div className={styles.cover} data-testid="cover-card" role="img" aria-label="示例封面图">
                        <svg viewBox="0 0 132 176" preserveAspectRatio="xMidYMid slice" aria-hidden="true">
                          <defs>
                            <linearGradient id="cw-sky" x1="0" y1="0" x2="0" y2="1">
                              <stop offset="0" stopColor="var(--lgi-muted)"/><stop offset=".48" stopColor="var(--lgi-reading-edge)"/><stop offset=".68" stopColor="var(--lgi-warning-soft)"/><stop offset="1" stopColor="var(--lgi-info)"/>
                            </linearGradient>
                            <linearGradient id="cw-water" x1="0" y1="0" x2="0" y2="1">
                              <stop offset="0" stopColor="var(--lgi-info-soft)"/><stop offset="1" stopColor="var(--lgi-info)"/>
                            </linearGradient>
                          </defs>
                          <rect width="132" height="176" fill="url(#cw-sky)"/>
                          <path d="M0 30 Q18 20 35 28 T70 25 T105 31 T145 24" fill="none" stroke="var(--lgi-reading-paper)" strokeWidth="8" opacity=".24"/>
                          <path d="M0 55 Q15 48 29 54 T58 50 T88 56 T120 51 T150 55" fill="none" stroke="var(--lgi-reading-paper)" strokeWidth="5" opacity=".28"/>
                          <path d="M0 84 L18 72 35 78 54 65 73 77 93 66 112 78 132 70 132 104 0 104Z" fill="var(--lgi-muted)" opacity=".8"/>
                          <path d="M0 96 Q22 91 44 96 T88 95 T132 92 L132 176 0 176Z" fill="url(#cw-water)"/>
                          <path d="M0 110 Q28 106 54 111 T104 110 T145 109 M0 127 Q20 124 42 128 T92 127 T145 126" fill="none" stroke="var(--lgi-reading-edge)" strokeWidth="1" opacity=".38"/>
                          <path d="M0 106 L132 106" stroke="var(--lgi-warning)" strokeWidth="2" opacity=".55"/>
                          <g fill="var(--lgi-ink)" opacity=".96">
                            <rect x="0" y="121" width="132" height="4"/><rect x="0" y="151" width="132" height="4"/>
                            <rect x="4" y="111" width="5" height="65"/><rect x="28" y="111" width="5" height="65"/>
                            <rect x="52" y="111" width="5" height="65"/><rect x="76" y="111" width="5" height="65"/>
                            <rect x="100" y="111" width="5" height="65"/><rect x="124" y="111" width="5" height="65"/>
                          </g>
                        </svg>
                        {!coverCompare ? <div className={styles.coverTitle}>{currentDraft.cover}</div> : null}
                        <span className={styles.coverTag}>示例图</span>
                      </div>
                      <button type="button" className={styles.coverCompare} onClick={() => setCoverCompare(value => !value)}>
                        {coverCompare ? '回到可编辑封面' : '对照原封面'}
                      </button>
                    </div>
                    <div className={styles.coverFields} data-testid="cover-fields">
                      <div>
                        <label className={styles.fieldLabel} htmlFor="cw-cover">封面大字 <span className={styles.count}>{Array.from(currentDraft.cover).length}/10</span></label>
                        <div className={styles.coverEntry}>
                          <input id="cw-cover" value={currentDraft.cover} maxLength={10} onChange={event => updateDraft({ cover: event.target.value })}/>
                          <div className={styles.candTools}>
                            <button type="button" onClick={() => previewOnly('换一批封面候选')}>换一批</button>
                            <button type="button" onClick={() => setInspectorTab('titles')}>找标题</button>
                          </div>
                        </div>
                        <div className={styles.coverCandidates}>
                          {currentDraft.coverCandidates.map(candidate => (
                            <button key={candidate} type="button" aria-pressed={currentDraft.cover === candidate}
                              onClick={() => updateDraft({ cover: candidate })}>{candidate} <small>{Array.from(candidate).length}</small></button>
                          ))}
                        </div>
                      </div>
                      <div>
                        <label className={styles.fieldLabel} htmlFor="cw-title">笔记标题 <span className={styles.count}>{Array.from(currentDraft.noteTitle).length}/20</span></label>
                        <div className={styles.fieldRow}>
                          <input id="cw-title" value={currentDraft.noteTitle} maxLength={20} onChange={event => updateDraft({ noteTitle: event.target.value })}/>
                          <ActionButton variant="outline" onClick={() => previewOnly('复制标题')}>复制标题</ActionButton>
                        </div>
                      </div>
                    </div>
                  </div>
                  <div className={styles.bodyHeading} data-testid="body-heading">
                    <h3>正文</h3>
                    <span>约 <b>{currentDraft.charCount}</b> 字 · <b>{currentDraft.paragraphCount}</b> 段 · 来自经历库 <b>{currentDraft.experienceCount}</b> 段</span>
                  </div>
                  <div className={styles.paragraphs} data-testid="draft-paragraphs">
                    {currentDraft.paragraphs.map(paragraph => paragraph.bridge ? (
                      <div className={styles.para} data-testid={paragraph.id === "p1" ? "first-paragraph" : undefined} data-paragraph-id={paragraph.id} key={paragraph.id}>
                        <div className={styles.paraLabel}>衔接</div>
                        <div className={styles.paraMain}>
                          <div className={styles.bridge}>
                            <div className={styles.bridgeHead}>
                              <span>一句话衔接产品</span><span className={styles.identityChip}>身份待确认</span>
                              <button type="button" onClick={() => previewOnly('这篇不植入')}>这篇不植入</button>
                            </div>
                            <div className={styles.bridgeBody}>
                              <p className={styles.bridgeQuestion}>示例妈妈和机构是什么关系？</p>
                              <div className={styles.bridgeOptions}>
                                <button type="button" onClick={() => previewOnly('选择：参加过课程的家长')}>参加过课程的家长</button>
                                <button type="button" onClick={() => previewOnly('选择：机构老师')}>机构老师</button>
                                <button type="button" onClick={() => previewOnly('选择：都不是，不提产品')}>都不是，不提产品</button>
                              </div>
                              <p className={styles.bridgeNote}>定了身份，简介里要写明这层关系。真实情况需要本人确认，预览中不写入正式资料。</p>
                              <div className={styles.bridgeOriginal}><span>原句</span>如果要在正文里提到服务，先确认真实关系与可以公开的信息。</div>
                            </div>
                          </div>
                        </div>
                      </div>
                    ) : (
                      <div className={styles.para} data-testid={paragraph.id === "p1" ? "first-paragraph" : undefined} data-paragraph-id={paragraph.id} key={paragraph.id}>
                        <div className={styles.paraLabel}>{paragraph.role}</div>
                        <div className={styles.paraMain}>
                          <p className={styles.paraText} contentEditable suppressContentEditableWarning role="textbox"
                            aria-label={paragraph.role + '段落'} spellCheck={false}
                            onBlur={event => updateParagraph(paragraph.id, event.currentTarget.textContent ?? '')}>{paragraph.text}</p>
                          <div className={styles.paraMeta}>
                            {paragraph.source ? <span className={styles.source}>{paragraph.source}</span> : <span/>}
                            <span className={styles.paraTools}>
                              {paragraph.alternatives?.length ? <button type="button" onClick={() => previewOnly('查看这一段的其他说法')}>换个说法</button> : null}
                              {paragraph.source ? <button type="button" onClick={event => { setExperienceTargetId(paragraph.id); openG5('experience-picker', event.currentTarget); }}>换一段经历</button> : null}
                              <button type="button" onClick={event => { setExperienceTargetId(paragraph.id); openG5('experience-picker', event.currentTarget); }}>后面插入经历</button>
                              <button type="button" onClick={() => updateDraft({ paragraphs: currentDraft.paragraphs.filter(item => item.id !== paragraph.id) })}>删除</button>
                            </span>
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                  <div className={styles.tagsRow}>
                    {currentDraft.tags.map((tag, index) => (
                      <span className={styles.tag} key={tag + '-' + index}>{tag}<button type="button" aria-label={'删除话题 ' + tag} onClick={() => updateDraft({ tags: currentDraft.tags.filter((_, i) => i !== index) })}>×</button></span>
                    ))}
                    <input aria-label="添加话题" placeholder="＋ 话题，回车" onKeyDown={event => {
                      if (event.key === 'Enter' && event.currentTarget.value.trim()) {
                        const value = event.currentTarget.value.trim();
                        updateDraft({ tags: [...currentDraft.tags, value.startsWith('#') ? value : '#' + value] });
                        event.currentTarget.value = '';
                      }
                    }}/>
                  </div>
                </div>
              </div>
              <footer className={styles.editorFoot}>
                <button type="button" className={styles.checkSummary} onClick={() => setInspectorTab('checks')}>
                  <i data-level="must"/>必改 <b>{currentDraft.must}</b><i data-level="warn"/>提醒 <b>{currentDraft.warn}</b>
                </button>
                <span className={styles.footerHint}>必改清零后才能复制</span>
                <span className={styles.grow}/>
                <ActionButton variant="outline" onClick={event => openG5('save-as-template', event.currentTarget)}>存为样板</ActionButton>
                <ActionButton variant="outline" disabled title="先把必改项目处理掉">复制到小红书</ActionButton>
              </footer>
            </section>

            <aside className={styles.inspector} aria-label="稿件检查器">
              <nav className={styles.inspectorTabs} role="tablist" aria-label="检查器视图" onKeyDown={event => {
                if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
                const tabs = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"]'));
                const index = tabs.indexOf(document.activeElement as HTMLButtonElement);
                if (index < 0) return;
                event.preventDefault();
                const next = event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1 : (index + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length;
                const nextTab = (['checks', 'materials', 'titles', 'preview'] as InspectorTab[])[next];
                if (nextTab) setInspectorTab(nextTab);
                tabs[next]?.focus();
              }}>
                {([['checks','检查'],['materials','素材'],['titles','标题'],['preview','预览']] as [InspectorTab,string][]).map(([id,label]) => (
                  <button key={id} id={'cw-inspector-tab-' + id} type="button" role="tab" tabIndex={inspectorTab === id ? 0 : -1}
                    aria-controls="cw-inspector-panel" aria-selected={inspectorTab === id} onClick={() => setInspectorTab(id)}>
                    {label}{id === 'checks' ? <b>{currentDraft.must + currentDraft.warn}</b> : null}
                  </button>
                ))}
                <button type="button" className={styles.mobileRail} aria-label="关闭稿件检查器" onClick={() => setInspectorDrawer(false)}>关闭</button>
              </nav>
              <div id="cw-inspector-panel" className={styles.inspectorScroll} role="tabpanel" aria-labelledby={'cw-inspector-tab-' + inspectorTab} tabIndex={0}>
                {inspectorTab === 'checks' ? (
                  <>
                    <div className={styles.checkGroup}>
                      <strong>必须先改 <b>{currentDraft.must}</b></strong>
                      <article>
                        <h3>账号身份还没确认</h3>
                        <p>衔接句要先确认示例妈妈和机构的关系，并在简介里写明。</p>
                        <div className={styles.checkActions}>
                          <button type="button" onClick={() => { setAssetOpen(true); setAssetKind('persona'); }}>去确认</button>
                          <button type="button" onClick={() => previewOnly('这篇不植入')}>这篇不植入</button>
                        </div>
                      </article>
                    </div>
                    <button type="button" className={styles.reminderToggle} onClick={() => previewOnly('展开提醒清单')}>
                      <span><i/>提醒 <b>{currentDraft.warn}</b></span><span aria-hidden="true">›</span>
                    </button>
                    <div className={styles.noticePanel}>复制到小红书时自动处理：段落之间补上空行（ㅤ），重复的话题只留一个。</div>
                  </>
                ) : null}
                {inspectorTab === 'materials' ? (
                  <><h3>本稿使用的材料</h3><p>经历和材料来源会在段落下方标注；这里展示的是隔离预览数据。</p><ActionButton variant="outline" onClick={openDesk}>打开材料桌</ActionButton></>
                ) : null}
                {inspectorTab === 'titles' ? (
                  <><h3>标题候选</h3>{currentDraft.coverCandidates.map(text => <button className={styles.titleCandidate} type="button" key={text} onClick={() => updateDraft({ noteTitle: text })}>{text}</button>)}</>
                ) : null}
                {inspectorTab === 'preview' ? (
                  <><h3>发布预览</h3><div className={styles.phonePreview}><strong>{currentDraft.noteTitle}</strong><p>{currentDraft.paragraphs.find(item => !item.bridge)?.text}</p><small>预览内容 · 合成数据</small></div></>
                ) : null}
              </div>
            </aside>
          </section>

          <section className={[styles.desk, deskSearchOpen ? styles.searchOpen : '', deskInspectorOpen ? styles.inspectorOpen : ''].filter(Boolean).join(' ')} hidden={mode !== 'desk'} aria-label="沉浸材料桌">
            <aside className={styles.deskSearch}>
              <div className={styles.deskSearchHead}><strong>找材料</strong><ActionButton variant="quiet" onClick={() => setMode('writing')}>返回写稿</ActionButton></div>
              <FindField label="搜索材料" value={deskSearch} onCommit={setDeskSearch} placeholder="搜索材料内容" trigger="debounce_250" />
              <SelectField label="材料类型" value={deskFilter} options={FILTER_OPTIONS} onChange={setDeskFilter} compact />
              <div className={styles.materialResults}>{deskVisibleItems.filter(item => item.kind !== 'frame').map(item => <button key={item.id} type="button" className={styles.materialResult} onClick={() => { setSelectedDeskId(item.id); setFocusedFrame(null); }}><strong>{item.title}</strong><span>{item.body}</span><small>演示材料 · 可放入画布</small></button>)}</div>
            </aside>
            <section className={styles.canvasRegion} aria-label="材料画布">
              <div className={styles.canvasToolbar}>
                <span>材料桌 <small>{deskItems.length} 个对象</small></span>
                <div><ActionButton className={styles.mobileRail} variant="quiet" onClick={() => setDeskSearchOpen(value => !value)}>找材料</ActionButton><ActionButton className={styles.mobileRail} variant="quiet" onClick={() => setDeskInspectorOpen(value => !value)}>检查器</ActionButton><ActionButton variant="outline" onClick={() => addDeskItem('frame')}>新选题框</ActionButton><ActionButton variant="outline" onClick={() => addDeskItem('note')}>新便签</ActionButton><ActionButton variant="quiet" onClick={() => { setZoom(1); setPan({ x: 30, y: 24 }); }}>看全部</ActionButton><ActionButton variant="quiet" onClick={() => setZoom(value => Math.max(.45, value / 1.15))}>−</ActionButton><span>{Math.round(zoom * 100)}%</span><ActionButton variant="quiet" onClick={() => setZoom(value => Math.min(2.4, value * 1.15))}>＋</ActionButton></div>
              </div>
              <div className={styles.canvasViewport} data-testid="canvas-viewport" onPointerDown={onCanvasPointerDown} onPointerMove={onCanvasPointerMove} onPointerUp={onCanvasPointerUp} onPointerCancel={onCanvasPointerUp} onWheel={onCanvasWheel} onDoubleClick={onCanvasDoubleClick} tabIndex={0} onKeyDown={event => { if (event.key === 'Escape') setFocusedFrame(null); }}>
                <div className={styles.canvasGrid} />
                <div className={styles.canvasWorld} style={{ transform: 'translate(' + pan.x + 'px,' + pan.y + 'px) scale(' + zoom + ')' }}>
                  {deskItems.map(item => <article key={item.id} data-canvas-item={item.id} className={[styles.deskCard, item.kind === 'frame' ? styles.frameCard : item.kind === 'note' ? styles.noteCard : styles.materialCard, selectedDeskId === item.id ? styles.selectedCard : '', focusedFrame === item.id ? styles.focusedCard : ''].join(' ')} style={{ left: item.x, top: item.y, width: item.width, minHeight: item.height } as CSSProperties} onClick={() => { setSelectedDeskId(item.id); if (item.kind === 'frame') setFocusedFrame(item.id); }} onKeyDown={event => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); setSelectedDeskId(item.id); if (item.kind === 'frame') setFocusedFrame(item.id); } }} tabIndex={0} aria-label={item.title}>
                    <header><strong>{item.title}</strong><small>{item.kind === 'frame' ? '选题框' : item.kind === 'note' ? '便签' : '材料卡'}</small></header>
                    <p>{item.body || '点击检查器补充内容。'}</p>
                    {item.kind === 'frame' ? <small>材料 {deskItems.filter(card => card.kind === 'material').length} 张 · 演示账号</small> : null}
                  </article>)}
                </div>
                <div className={styles.canvasHint}>拖动空白处平移 · Ctrl/⌘ + 滚轮缩放 · 双击空白处新建便签 · F1–F4 聚焦选题框</div>
              </div>
            </section>
            <aside className={styles.deskInspector}>
              <div className={styles.deskInspectorHead}><strong>检查器</strong><small>本地演示</small></div>
              {selectedDeskItem ? <><label>名称<input value={selectedDeskItem.title} onChange={event => setDeskItems(items => items.map(item => item.id === selectedDeskItem.id ? { ...item, title: event.target.value } : item))} /></label><label>内容<textarea value={selectedDeskItem.body} onChange={event => setDeskItems(items => items.map(item => item.id === selectedDeskItem.id ? { ...item, body: event.target.value } : item))} /></label><div className={styles.inspectorActions}><ActionButton variant="outline" onClick={() => previewOnly('放入当前稿件')}>放入当前稿件</ActionButton><ActionButton variant="danger" onClick={() => { setDeskItems(items => items.filter(item => item.id !== selectedDeskItem.id)); setSelectedDeskId(null); }}>移出画布</ActionButton></div></> : <EmptyState kind="first_use" title="选择一个材料或选题框">选择对象后，在这里查看和编辑内容。</EmptyState>}
              <ActionButton className={styles.fullWidth} variant="primary" onClick={() => previewOnly('用这组材料开稿')}>用这组材料开稿</ActionButton>
            </aside>
          </section>
        </main>
      </OpsFrame>

      <dialog ref={dialogRef} className={styles.assetDialog} aria-label="创作资产库" onCancel={event => { event.preventDefault(); setAssetOpen(false); }} onClose={() => setAssetOpen(false)} onClick={event => { if (event.target === event.currentTarget) setAssetOpen(false); }}>
        <header className={styles.assetHead}><div><small>CREATION ASSETS · PREVIEW</small><h2>资产库</h2></div><ActionButton variant="outline" aria-label="关闭资产库" onClick={() => setAssetOpen(false)}>关闭</ActionButton></header>
        <div className={styles.assetBody}>
          <nav className={styles.assetIndex} aria-label="资产类型">{ASSETS.map(asset => <button key={asset.id} type="button" aria-current={assetKind === asset.id ? 'page' : undefined} onClick={() => { setAssetKind(asset.id); setAssetSelection(0); setProductEditing(false); }}><strong>{asset.label}</strong><small>{asset.description}</small></button>)}</nav>
          <section className={styles.assetList} aria-label="资产列表">
            <FindField label="搜索资产" value={assetSearch} onCommit={setAssetSearch} placeholder="搜索名称或内容" trigger="debounce_250" />
            <div className={styles.assetRows}>{assetItems.map((item,index) => <button key={item.title} type="button" aria-current={assetSelection === index ? 'true' : undefined} onClick={() => setAssetSelection(index)}><strong>{item.title}</strong><small>{item.meta}</small><p>{item.body}</p></button>)}</div>
            {!assetItems.length ? <EmptyState title="当前类型没有匹配内容">调整搜索词，或切换其他资产类型。</EmptyState> : null}
          </section>
          <section className={styles.assetDetail} aria-label="资产详情">
            {selectedAssetItem ? <><small>{ASSETS.find(item => item.id === assetKind)?.label}</small><h3>{selectedAssetItem.title}</h3><p>{selectedAssetItem.body}</p><InlineNotice tone="info">这里展示的是合成预览数据，不会保存到正式资产库。</InlineNotice>{assetKind === 'product' ? (productEditing ? <div className={styles.productEditing}><label>规范名称<input value={productDraft} onChange={event => setProductDraft(event.target.value)} /></label><label>适用边界<textarea defaultValue="仅供演示；实际信息须由机构确认。" /></label><div><ActionButton variant="quiet" onClick={() => setProductEditing(false)}>取消</ActionButton><ActionButton variant="outline" onClick={() => { setProductEditing(false); previewOnly('保存产品事实卡'); }}>保存（仅预览）</ActionButton></div></div> : <ActionButton variant="outline" onClick={() => setProductEditing(true)}>编辑</ActionButton>) : <ActionButton variant="outline" onClick={() => previewOnly('编辑资产')}>编辑资产</ActionButton>}</> : <EmptyState title="选择一个资产">从列表选择一条资产，查看详情。</EmptyState>}
          </section>
        </div>
        <footer className={styles.assetFoot}><span>合成数据 · 不连接正式资产 API</span><ActionButton variant="quiet" onClick={() => setAssetOpen(false)}>返回工作台</ActionButton></footer>
      </dialog>

      {tourOpen ? <CreatorWorkbenchPreviewTour onClose={() => setTourOpen(false)} /> : null}
      <CreatorWorkbenchPreviewG5States kind={g5Dialog} onClose={() => setG5Dialog(null)} draftTitle={currentDraft.title} paragraphRoles={currentDraft.paragraphs.map(item => item.role)} onUseExperience={useSyntheticExperience} onPreviewOnly={previewOnly} />
      {notice ? <div className={styles.noticeToast}><InlineNotice tone="info">{notice}</InlineNotice><button type="button" aria-label="关闭提示" onClick={() => setNotice('')}>关闭</button></div> : null}
    </div>
  );
}
