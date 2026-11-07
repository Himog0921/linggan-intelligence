import { useEffect, useRef, useState, type CSSProperties, type PointerEvent, type WheelEvent } from 'react';
import { OpsFrame } from '../ops/OpsFrame';
import { ActionButton } from '../../shared/ActionButton';
import { EmptyState } from '../../shared/EmptyState';
import { FindField } from '../../shared/FindField';
import { InlineNotice } from '../../shared/InlineNotice';
import { SelectField } from '../../shared/SelectField';
import styles from './CreatorWorkbenchPreviewPage.module.css';

type InspectorTab = 'checks' | 'materials' | 'titles' | 'preview';
type AssetKind = 'persona' | 'experience' | 'product' | 'bridge' | 'sample' | 'style';
type Paragraph = { id: string; role: string; text: string };
type Draft = {
  id: string; title: string; account: string; state: string; must: number; warn: number;
  angle: string; cover: string; noteTitle: string; paragraphs: Paragraph[];
};
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

function makeParagraphs(prefix: string): Paragraph[] {
  const rows: Paragraph[] = [
    ['钩子', '前几天整理旧照片时，我突然发现，很多当时觉得过不去的事情，现在回头看已经有了新的答案。'],
    ['自问', '我以前总想一次把所有问题都解决，结果越着急，越容易忽略眼前真正需要处理的那一件事。'],
    ['过去的我', '那段时间，我习惯把每一次不顺利都归结为自己做得不够好，也很少停下来看看具体发生了什么。'],
    ['场景', '有一次周末，我们原本安排了很多事情，最后只完成了其中两件，反而比预想中更轻松。'],
    ['转折', '后来我开始把目标拆小，先把最重要的一步做好，再决定下一步要不要继续。'],
    ['观点', '真正有效的改变，往往不是突然变得很厉害，而是慢慢找到一种能够长期坚持的办法。'],
  ];
  return rows.map((row, index) => ({ id: prefix + '-' + index, role: row[0], text: row[1] }));
}

const INITIAL_DRAFTS: Draft[] = [
  { id: 'd1', title: '先把眼前这一件事做好', account: '示例账号 · 试新', state: '必须先改', must: 1, warn: 5, angle: '从一个具体场景讲清楚自己的变化', cover: '先把眼前这一件事做好', noteTitle: '先把眼前这一件事做好', paragraphs: makeParagraphs('d1') },
  { id: 'd2', title: '我后来不再急着给答案', account: '示例账号 · 复用', state: '有提醒', must: 0, warn: 2, angle: '复盘一次沟通中的误区', cover: '先听完，再给答案', noteTitle: '我后来不再急着给答案', paragraphs: makeParagraphs('d2') },
  { id: 'd3', title: '一次普通周末的安排', account: '示例账号 · 试新', state: '已检查', must: 0, warn: 0, angle: '把复杂安排拆成小步骤', cover: '周末只做两件事', noteTitle: '一次普通周末的安排', paragraphs: makeParagraphs('d3') },
  { id: 'd4', title: '有些变化是慢慢发生的', account: '示例账号 · 复用', state: '有提醒', must: 0, warn: 3, angle: '从持续的小变化展开', cover: '变化其实很具体', noteTitle: '有些变化是慢慢发生的', paragraphs: makeParagraphs('d4') },
  { id: 'd5', title: '给自己留一点调整空间', account: '示例账号 · 试新', state: '已复制', must: 0, warn: 0, angle: '记录一次临时调整', cover: '计划之外也没关系', noteTitle: '给自己留一点调整空间', paragraphs: makeParagraphs('d5') },
  { id: 'd6', title: '一件小事带来的提醒', account: '示例账号 · 复用', state: '已登记发布', must: 0, warn: 0, angle: '用一个具体细节收尾', cover: '从一件小事开始', noteTitle: '一件小事带来的提醒', paragraphs: makeParagraphs('d6') },
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
  { value: 'note', label: '笔记摘录' },
  { value: 'comment', label: '评论摘录' },
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
  const [deskSearch, setDeskSearch] = useState('');
  const [deskFilter, setDeskFilter] = useState<(typeof FILTER_OPTIONS)[number]['value']>('all');
  const [deskItems, setDeskItems] = useState(INITIAL_DESK_ITEMS);
  const [selectedDeskId, setSelectedDeskId] = useState<string | null>('f1');
  const [focusedFrame, setFocusedFrame] = useState<string | null>(null);
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 30, y: 24 });
  const [queueDrawer, setQueueDrawer] = useState(false);
  const [inspectorDrawer, setInspectorDrawer] = useState(false);
  const [assetTab, setAssetTab] = useState<AssetKind>('persona');
  const assetTrigger = useRef<HTMLButtonElement>(null);
  const dialogRef = useRef<HTMLDialogElement>(null);
  const canvasRef = useRef<HTMLDivElement>(null);
  const pointerRef = useRef<{ x: number; y: number; panX: number; panY: number } | null>(null);
  const currentDraft = drafts.find(item => item.id === draftId) ?? drafts[0]!;

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (assetOpen && !dialog.open) dialog.showModal();
    if (!assetOpen && dialog.open) dialog.close();
  }, [assetOpen]);

  useEffect(() => {
    if (!assetOpen) assetTrigger.current?.focus({ preventScroll: true });
  }, [assetOpen]);

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
  const openDesk = () => { setMode('desk'); setFocusedFrame(null); setZoom(1); setPan({ x: 30, y: 24 }); };
  const addDeskItem = (kind: DeskItem['kind']) => {
    const id = 'local-' + (deskItems.length + 1);
    const item: DeskItem = { id, kind, title: kind === 'frame' ? '新选题框' : '新便签', body: '', x: 180 + deskItems.length * 24, y: 180 + deskItems.length * 18, width: kind === 'frame' ? 300 : 230, height: kind === 'frame' ? 188 : 132 };
    setDeskItems(items => [...items, item]);
    setSelectedDeskId(id);
    previewOnly(kind === 'frame' ? '已在本地画布添加选题框' : '已在本地画布添加便签');
  };

  const onCanvasPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || (event.target as HTMLElement).closest('button,input,textarea,.deskCard')) return;
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
  const onCanvasDoubleClick = (event: React.MouseEvent<HTMLDivElement>) => {
    if ((event.target as HTMLElement).closest('button,.deskCard')) return;
    addDeskItem('note');
  };
  const deskVisibleItems = deskItems.filter(item => {
    const matchesText = !deskSearch || (item.title + ' ' + item.body).toLowerCase().includes(deskSearch.toLowerCase());
    const matchesType = deskFilter === 'all' || (deskFilter === 'idea' ? item.kind === 'note' : item.kind === 'material');
    return matchesText && matchesType;
  });
  const selectedDeskItem = deskItems.find(item => item.id === selectedDeskId) ?? null;
  const assetItems = ASSET_ROWS[assetKind].filter(item => !assetSearch || (item.title + item.body).includes(assetSearch));

  return (
    <div className={styles.root} data-preview-only="true">
      <OpsFrame section="creation" domain="preview" scope={<span>创作演示</span>} immersive={mode === 'desk'}
        tabs={<>
          <button type="button" aria-current={mode === 'writing' ? 'page' : undefined} onClick={() => setMode('writing')}>写稿</button>
          <button type="button" aria-current={mode === 'desk' ? 'page' : undefined} onClick={openDesk}>材料桌</button>
        </>}
        meta={<>
          <span className={styles.demoMarker}>DEMO · 合成数据</span>
          <span className="v7-vr" aria-hidden="true" />
          <span className={styles.metaCount}>今日 <b>6</b> 篇 · 待完成 <b>5</b></span>
          <ActionButton ref={assetTrigger} variant="outline" onClick={() => { setAssetOpen(true); setAssetKind('persona'); setAssetSelection(0); }}>资产库</ActionButton>
          <ActionButton variant="quiet" onClick={() => previewOnly('使用路径')}>使用路径</ActionButton>
        </>}>
        <main className={styles.main} aria-label="创作工作台">
          <section className={styles.write} hidden={mode !== 'writing'}>
            <aside className={styles.queue} aria-label="稿件队列">
              <div className={styles.queueHead}>
                <div><small>今日 · 10月27日 周一</small><strong>稿件队列</strong></div>
                <ActionButton variant="outline" onClick={() => { const id = 'new-' + (drafts.length + 1); setDrafts(items => [...items, { ...INITIAL_DRAFTS[0]!, id, title: '新建稿件', paragraphs: makeParagraphs(id) }]); setDraftId(id); previewOnly('已建立本地演示稿件'); }}>新选题</ActionButton>
              </div>
              <ActionButton className={styles.queueDeskButton} variant="quiet" onClick={openDesk}>选题与材料 <span>4 个选题</span></ActionButton>
              <div className={styles.queueScroll}>
                <div className={styles.groupTitle}>选题中 · 还没开稿 <b>1</b></div>
                <div className={styles.groupTitle}>稿件 <b>{drafts.length}</b></div>
                {drafts.map(item => <button key={item.id} type="button" className={styles.draftItem} aria-current={item.id === draftId ? 'true' : undefined} onClick={() => { setDraftId(item.id); setInspectorTab('checks'); setQueueDrawer(false); }}>
                  <strong>{item.title}</strong><small>{item.account}</small><span><i data-level={item.must ? 'must' : item.warn ? 'warn' : 'ok'} />{item.must ? '必改 ' + item.must : item.state} · 提醒 {item.warn}</span>
                </button>)}
              </div>
            </aside>

            <section className={styles.editor} aria-label="稿件编辑区">
              <header className={styles.editorHead}>
                <button type="button" className={styles.mobileRail} onClick={() => setQueueDrawer(value => !value)} aria-expanded={queueDrawer}>稿件</button>
                <strong>{currentDraft.title}</strong>
                <span className={styles.statusChip}>{currentDraft.account}</span>
                <span className={styles.editorMeta}>试新 · 样板 YB-0001 · 风格：合成示例</span>
              </header>
              <div className={styles.editorScroll}>
                <div className={styles.editorInner}>
                  <div className={styles.intentRow}><b>选题意图</b><span>{currentDraft.angle}</span><button type="button" onClick={() => previewOnly('关联为同一选题')}>关联选题</button></div>
                  <div className={styles.coverRow}>
                    <div className={styles.coverPlaceholder} role="img" aria-label="演示封面图"><span>封面</span><small>合成素材</small></div>
                    <div className={styles.coverFields}>
                      <label>封面大字<input value={currentDraft.cover} maxLength={10} onChange={event => updateDraft({ cover: event.target.value })} /></label>
                      <div className={styles.candidateRow}><button type="button" aria-pressed={currentDraft.cover === '先把眼前这一件事做好'} onClick={() => updateDraft({ cover: '先把眼前这一件事做好' })}>先把眼前这一件事做好</button><button type="button" onClick={() => previewOnly('换一批封面候选')}>换一批</button><button type="button" onClick={() => setInspectorTab('titles')}>找标题</button></div>
                      <label>笔记标题<input value={currentDraft.noteTitle} maxLength={20} onChange={event => updateDraft({ noteTitle: event.target.value })} /></label>
                    </div>
                  </div>
                  <div className={styles.bodyHeading}><strong>正文</strong><small>{currentDraft.paragraphs.reduce((sum, item) => sum + Array.from(item.text).length, 0)} 字 · {currentDraft.paragraphs.length} 段</small></div>
                  {currentDraft.paragraphs.map((paragraph, index) => <div className={styles.paragraph} key={paragraph.id}>
                    <span>{paragraph.role}</span>
                    <textarea aria-label={'正文第 ' + (index + 1) + ' 段'} value={paragraph.text} onChange={event => updateParagraph(paragraph.id, event.target.value)} onCompositionStart={() => {}} onCompositionEnd={() => {}} />
                  </div>)}
                  <ActionButton variant="outline" onClick={() => updateDraft({ paragraphs: [...currentDraft.paragraphs, { id: currentDraft.id + '-p' + (currentDraft.paragraphs.length + 1), role: '新段落', text: '' }] })}>继续写一段</ActionButton>
                </div>
              </div>
              <footer className={styles.editorFoot}>
                <span><i data-level="must" /> 必改 {currentDraft.must}　<i data-level="warn" /> 提醒 {currentDraft.warn}　<small>演示数据，不代表正式检查结果</small></span>
                <ActionButton variant="outline" onClick={() => previewOnly('存为样板')}>存为样板</ActionButton>
                <ActionButton variant="primary" onClick={() => previewOnly('复制到小红书')}>复制到小红书</ActionButton>
              </footer>
            </section>

            <aside className={styles.inspector} aria-label="稿件检查器">
              <nav className={styles.inspectorTabs} aria-label="检查器视图">
                {([['checks','检查'],['materials','素材'],['titles','标题'],['preview','预览']] as [InspectorTab,string][]).map(([id,label]) => <button key={id} type="button" role="tab" aria-selected={inspectorTab === id} onClick={() => setInspectorTab(id)}>{label}{id === 'checks' ? <b>{currentDraft.must + currentDraft.warn}</b> : null}</button>)}
              </nav>
              <div className={styles.inspectorScroll}>
                {inspectorTab === 'checks' ? <><div className={styles.checkGroup}><strong>必须先改 <b>{currentDraft.must}</b></strong><article><h3>账号身份信息待确认</h3><p>正式检查需要账号身份、机构关系和简介确认回执。</p><button type="button" onClick={() => { setAssetOpen(true); setAssetKind('persona'); }}>查看账号身份</button></article></div><div className={styles.checkGroup}><strong>提醒 <b>{currentDraft.warn}</b></strong><p>提醒项目仅为合成展示，正式规则以服务端检查结果为准。</p></div></> : null}
                {inspectorTab === 'materials' ? <><h3>本稿使用的材料</h3><p>材料来源和使用资格以正式服务端回执为准。</p><ActionButton variant="outline" onClick={openDesk}>打开材料桌</ActionButton></> : null}
                {inspectorTab === 'titles' ? <><h3>标题候选</h3>{['先把眼前这一件事做好','计划之外也没关系','变化其实很具体'].map(text => <button className={styles.titleCandidate} type="button" key={text} onClick={() => updateDraft({ noteTitle: text })}>{text}</button>)}</> : null}
                {inspectorTab === 'preview' ? <><h3>发布预览</h3><div className={styles.phonePreview}><strong>{currentDraft.noteTitle}</strong><p>{currentDraft.paragraphs[0]?.text}</p><small>预览内容 · 合成数据</small></div></> : null}
              </div>
              <button type="button" className={styles.mobileRail} onClick={() => setInspectorDrawer(value => !value)} aria-expanded={inspectorDrawer}>收起检查器</button>
            </aside>
          </section>

          <section className={styles.desk} hidden={mode !== 'desk'} aria-label="沉浸材料桌">
            <aside className={styles.deskSearch}>
              <div className={styles.deskSearchHead}><strong>找材料</strong><ActionButton variant="quiet" onClick={() => setMode('writing')}>返回写稿</ActionButton></div>
              <FindField label="搜索材料" value={deskSearch} onCommit={setDeskSearch} placeholder="搜索材料内容" trigger="debounce_250" />
              <SelectField label="材料类型" value={deskFilter} options={FILTER_OPTIONS} onChange={setDeskFilter} compact />
              <div className={styles.materialResults}>{deskVisibleItems.filter(item => item.kind !== 'frame').map(item => <button key={item.id} type="button" className={styles.materialResult} onClick={() => { setSelectedDeskId(item.id); setFocusedFrame(null); }}><strong>{item.title}</strong><span>{item.body}</span><small>演示材料 · 可放入画布</small></button>)}</div>
            </aside>
            <section className={styles.canvasRegion} aria-label="材料画布">
              <div className={styles.canvasToolbar}>
                <span>材料桌 <small>{deskItems.length} 个对象</small></span>
                <div><ActionButton variant="outline" onClick={() => addDeskItem('frame')}>新选题框</ActionButton><ActionButton variant="outline" onClick={() => addDeskItem('note')}>新便签</ActionButton><ActionButton variant="quiet" onClick={() => { setZoom(1); setPan({ x: 30, y: 24 }); }}>看全部</ActionButton><ActionButton variant="quiet" onClick={() => setZoom(value => Math.max(.45, value / 1.15))}>−</ActionButton><span>{Math.round(zoom * 100)}%</span><ActionButton variant="quiet" onClick={() => setZoom(value => Math.min(2.4, value * 1.15))}>＋</ActionButton></div>
              </div>
              <div className={styles.canvasViewport} ref={canvasRef} onPointerDown={onCanvasPointerDown} onPointerMove={onCanvasPointerMove} onPointerUp={onCanvasPointerUp} onPointerCancel={onCanvasPointerUp} onWheel={onCanvasWheel} onDoubleClick={onCanvasDoubleClick} tabIndex={0} onKeyDown={event => { if (event.key === 'Escape') setFocusedFrame(null); }}>
                <div className={styles.canvasGrid} />
                <div className={styles.canvasWorld} style={{ transform: 'translate(' + pan.x + 'px,' + pan.y + 'px) scale(' + zoom + ')' }}>
                  {deskItems.map(item => <article key={item.id} className={[styles.deskCard, item.kind === 'frame' ? styles.frameCard : item.kind === 'note' ? styles.noteCard : styles.materialCard, selectedDeskId === item.id ? styles.selectedCard : '', focusedFrame === item.id ? styles.focusedCard : ''].join(' ')} style={{ left: item.x, top: item.y, width: item.width, minHeight: item.height } as CSSProperties} onClick={() => { setSelectedDeskId(item.id); if (item.kind === 'frame') setFocusedFrame(item.id); }} tabIndex={0} aria-label={item.title}>
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
              <ActionButton variant="primary" block onClick={() => previewOnly('用这组材料开稿')}>用这组材料开稿</ActionButton>
            </aside>
          </section>
        </main>
      </OpsFrame>

      <dialog ref={dialogRef} className={styles.assetDialog} aria-label="创作资产库" onCancel={event => { event.preventDefault(); setAssetOpen(false); }} onClose={() => setAssetOpen(false)} onClick={event => { if (event.target === event.currentTarget) setAssetOpen(false); }}>
        <header className={styles.assetHead}><div><small>CREATION ASSETS · PREVIEW</small><h2>资产库</h2></div><ActionButton variant="outline" aria-label="关闭资产库" onClick={() => setAssetOpen(false)}>关闭</ActionButton></header>
        <div className={styles.assetBody}>
          <nav className={styles.assetIndex} aria-label="资产类型">{ASSETS.map(asset => <button key={asset.id} type="button" aria-current={assetKind === asset.id ? 'page' : undefined} onClick={() => { setAssetKind(asset.id); setAssetSelection(0); }}><strong>{asset.label}</strong><small>{asset.description}</small></button>)}</nav>
          <section className={styles.assetList} aria-label="资产列表">
            <FindField label="搜索资产" value={assetSearch} onCommit={setAssetSearch} placeholder="搜索名称或内容" trigger="debounce_250" />
            <div className={styles.assetRows}>{assetItems.map((item,index) => <button key={item.title} type="button" aria-current={assetSelection === index ? 'true' : undefined} onClick={() => setAssetSelection(index)}><strong>{item.title}</strong><small>{item.meta}</small><p>{item.body}</p></button>)}</div>
            {!assetItems.length ? <EmptyState title="当前类型没有匹配内容">调整搜索词，或切换其他资产类型。</EmptyState> : null}
          </section>
          <section className={styles.assetDetail} aria-label="资产详情">
            {assetItems[assetSelection] ? <><small>{ASSETS.find(item => item.id === assetKind)?.label}</small><h3>{assetItems[assetSelection]!.title}</h3><p>{assetItems[assetSelection]!.body}</p><InlineNotice tone="info">这里展示的是合成预览数据，不会保存到正式资产库。</InlineNotice><ActionButton variant="outline" onClick={() => previewOnly('编辑资产')}>编辑资产</ActionButton></> : <EmptyState title="选择一个资产">从列表选择一条资产，查看详情。</EmptyState>}
          </section>
        </div>
        <footer className={styles.assetFoot}><span>合成数据 · 不连接正式资产 API</span><ActionButton variant="quiet" onClick={() => setAssetOpen(false)}>返回工作台</ActionButton></footer>
      </dialog>

      {notice ? <div className={styles.noticeToast}><InlineNotice tone="info">{notice}</InlineNotice><button type="button" aria-label="关闭提示" onClick={() => setNotice('')}>关闭</button></div> : null}
    </div>
  );
}
