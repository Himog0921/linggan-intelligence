import * as React from 'react';

export type GoldMasterMode = 'writing' | 'desk';
export type GoldMasterRightTab = 'check' | 'material' | 'title' | 'preview';
export type GoldMasterAssetTab = 'persona' | 'experience' | 'product' | 'bridge' | 'sample' | 'style';
export type GoldMasterDialog = 'path' | 'tour' | 'save-sample' | 'account-pop' | 'pick-experience';
export type GoldMasterDeskPanel = 'desk-frame-editor' | 'desk-card-reader' | 'desk-card-role' | 'desk-card-memo';

export interface GoldMasterBundle {
  referenceSha256: string;
  writeDefault: string;
  writeDrafts: Record<string, string>;
  rightTabs: Record<Exclude<GoldMasterRightTab, 'check'>, string>;
  deskDefault: string;
  deskFocus: Record<string, string>;
  deskPanels: Record<string, string>;
  assets: Record<GoldMasterAssetTab, string>;
  dialogs: Record<GoldMasterDialog, string>;
}

interface AppState {
  mode: 'writing' | 'desk' | 'new-draft';
  draftId: string;
  rightTab: GoldMasterRightTab;
  asset: GoldMasterAssetTab | null;
  dialog: GoldMasterDialog | null;
  queueDrawerOpen: boolean;
  inspectorDrawerOpen: boolean;
  deskSearchOpen: boolean;
  deskFocusId: string | null;
  deskMini: GoldMasterDeskPanel | null;
  zoom: number;
  panX: number;
  panY: number;
  message: string;
}

interface Props {
  snapshots: GoldMasterBundle;
  onModeChange: (mode: GoldMasterMode) => void;
}

export interface CreatorWorkbenchGoldMasterHandle {
  dispatchAction: (action: string, trigger?: HTMLElement) => void;
}

const ASSET_KINDS: GoldMasterAssetTab[] = ['persona', 'experience', 'product', 'bridge', 'sample', 'style'];
const RIGHT_TABS: GoldMasterRightTab[] = ['check', 'material', 'title', 'preview'];
const DRAFT_IDS = ['d1', 'd2', 'd3', 'd4', 'd5', 'd6'];
const isAsset = (value: string): value is GoldMasterAssetTab => ASSET_KINDS.includes(value as GoldMasterAssetTab);
const isRightTab = (value: string): value is GoldMasterRightTab => RIGHT_TABS.includes(value as GoldMasterRightTab);
const isDraft = (value: string): boolean => DRAFT_IDS.includes(value);

function convertHtmlNode(node: Node, key: string, edits: Record<string, string>): React.ReactNode {
  if (node.nodeType === Node.TEXT_NODE) return node.textContent || '';
  if (node.nodeType !== Node.ELEMENT_NODE) return null;
  const element = node as HTMLElement;
  const tag = element.localName;
  const props: Record<string, unknown> = { key };
  for (let i = 0; i < element.attributes.length; i++) {
    const attr = element.attributes.item(i);
    if (!attr) continue;
    const n = attr.name, v = attr.value;
    if (n === 'class') props.className = v;
    else if (n === 'for') props.htmlFor = v;
    else if (n === 'tabindex') props.tabIndex = Number(v);
    else if (n === 'readonly') props.readOnly = true;
    else if (n === 'checked') props.defaultChecked = true;
    else if (n === 'selected') props.defaultSelected = true;
    else if (n === 'open' && tag === 'details') props.open = true;
    else if (['disabled', 'hidden', 'multiple', 'required'].includes(n)) props[n] = true;
    else if (n === 'value' && (tag === 'input' || tag === 'textarea')) props.defaultValue = v;
    else if (n === 'style') {
      const styles: Record<string, string | number> = {};
      const decl = element.style;
      for (let j = 0; j < decl.length; j++) {
        const cssName = decl.item(j);
        const val = decl.getPropertyValue(cssName);
        const camel = cssName.startsWith('--') ? cssName : cssName.replace(/-([a-z])/g, (_match, c: string) => c.toUpperCase());
        styles[camel] = val;
      }
      props.style = styles;
    } else if (n === 'viewbox') props.viewBox = v;
    else if (n === 'stroke-linecap') props.strokeLinecap = v;
    else if (n === 'stroke-linejoin') props.strokeLinejoin = v;
    else if (n === 'stroke-width') props.strokeWidth = v;
    else if (n === 'fill-rule') props.fillRule = v;
    else if (n === 'clip-rule') props.clipRule = v;
    else if (n === 'contenteditable') props.contentEditable = v === 'true';
    else if (n === 'maxlength') props.maxLength = Number(v);
    else if (n === 'autocomplete') props.autoComplete = v;
    else if (n === 'spellcheck') props.spellCheck = v !== 'false';
    else if (n === 'inputmode') props.inputMode = v;
    else if (n === 'colspan') props.colSpan = Number(v);
    else if (n === 'rowspan') props.rowSpan = Number(v);
    else if (n === 'srcset') props.srcSet = v;
    else if (n === 'novalidate') props.noValidate = true;
    else if (n === 'autofocus') props.autoFocus = true;
    else if (n === 'value' && tag !== 'input') props.value = v;
    else if (n !== 'xmlns') props[n] = v;
  }
  const editableKey = tag === 'textarea' || tag === 'input'
    ? element.getAttribute('data-paragraph') || element.getAttribute('data-pid') || element.id ||
      (element.closest('[data-pid]')?.getAttribute('data-pid') ? 'paragraph:' + element.closest('[data-pid]')!.getAttribute('data-pid') : null)
    : null;
  if (editableKey && Object.prototype.hasOwnProperty.call(edits, editableKey)) props.defaultValue = edits[editableKey];
  if (element.getAttribute('contenteditable') === 'true') {
    const identity = element.getAttribute('data-pid') || element.id || key;
    if (Object.prototype.hasOwnProperty.call(edits, 'editable:' + identity)) {
      return React.createElement(tag, props, edits['editable:' + identity]);
    }
  }
  if (tag === 'textarea') {
    if (props.defaultValue === undefined) props.defaultValue = element.textContent || '';
    return React.createElement('textarea', props);
  }
  if (['input', 'img', 'path', 'circle', 'rect', 'line', 'br', 'hr'].includes(tag)) return React.createElement(tag, props);
  const children = Array.from(element.childNodes).map((child, index) => convertHtmlNode(child, key + '/' + index, edits));
  return React.createElement(tag, props, ...children);
}

function parseFragment(markup: string): Document {
  return new DOMParser().parseFromString(markup, 'text/html');
}

function AssetOverlayPreview({ html, edits }: { html: string; edits: Record<string, string> }) {
  const parsed = parseFragment(html);
  parsed.querySelector('.assets-modal')?.setAttribute('data-testid', 'assets-modal');
  return <div className="overlay-mount">{Array.from(parsed.body.children).map((element, index) => convertHtmlNode(element, 'overlay/' + index, edits))}</div>;
}

function SyntheticFeedback({ text }: { text: string }) {
  return text ? <div className="preview-feedback" role="status" aria-live="polite">预览 · {text}</div> : null;
}

export class CreatorWorkbenchGoldMasterAdapter extends React.Component<Props, AppState> implements CreatorWorkbenchGoldMasterHandle {
  override state: AppState = {
    mode: 'writing', draftId: 'd1', rightTab: 'check', asset: null, dialog: null,
    queueDrawerOpen: false, inspectorDrawerOpen: false, deskSearchOpen: false,
    deskFocusId: null, deskMini: null, zoom: 1, panX: 320, panY: 161.156, message: '',
  };
  private editorScroll = 0;
  private queueScroll = 0;
  private lastFocused: HTMLElement | null = null;
  private editsByDraft: Record<string, Record<string, string>> = {};
  private worldDrag: { x: number; y: number; px: number; py: number } | null = null;

  override componentDidMount() {
    window.addEventListener('keydown', this.onGlobalKeyDown);
    this.props.onModeChange('writing');
  }

  override componentWillUnmount() {
    window.removeEventListener('keydown', this.onGlobalKeyDown);
  }

  override componentDidUpdate(_previousProps: Props, previousState: AppState) {
    if (previousState.mode !== this.state.mode) {
      this.props.onModeChange(this.state.mode === 'desk' ? 'desk' : 'writing');
    }
    if (previousState.mode === 'writing' && this.state.mode === 'writing' &&
        previousState.draftId === this.state.draftId && this.state.asset === null && this.state.dialog === null) {
      const editor = document.getElementById('edscroll');
      if (editor && editor.scrollTop === 0 && this.editorScroll > 0) editor.scrollTop = this.editorScroll;
    }
    if (previousState.mode !== this.state.mode && this.state.mode === 'writing') {
      window.requestAnimationFrame(() => {
        const editor = document.getElementById('edscroll');
        if (editor) editor.scrollTop = this.editorScroll;
        const queue = document.getElementById('qlist');
        if (queue) queue.scrollTop = this.queueScroll;
        if (this.lastFocused?.isConnected) this.lastFocused.focus();
        else document.querySelector<HTMLElement>('[data-act="deskOpen"]')?.focus();
      });
    }
    if ((!previousState.asset && this.state.asset) || (!previousState.dialog && this.state.dialog)) {
      window.requestAnimationFrame(() => {
        document.querySelector<HTMLElement>('#overlay [data-act="assetsClose"], #overlay [data-act="modalClose"], #overlay [data-act="popClose"]')?.focus();
      });
    }
    if ((previousState.asset && !this.state.asset) || (previousState.dialog && !this.state.dialog)) {
      window.requestAnimationFrame(() => {
        if (this.lastFocused?.isConnected) this.lastFocused.focus();
        else document.querySelector<HTMLElement>('[data-act="openAssets"], [data-act="path"], [data-act="tour"]')?.focus();
      });
    }
    if (this.state.mode === 'desk') this.paintDesk();
  }

  private storeScroll() {
    this.editorScroll = document.getElementById('edscroll')?.scrollTop || 0;
    this.queueScroll = document.getElementById('qlist')?.scrollTop || 0;
  }

  private onGlobalKeyDown = (event: KeyboardEvent) => {
    if (event.key === 'Tab' && (this.state.asset || this.state.dialog)) {
      const overlay = document.querySelector<HTMLElement>('#overlay');
      const controls = Array.from(overlay?.querySelectorAll<HTMLElement>('button:not([disabled]),a[href],input:not([disabled]),textarea:not([disabled]),select:not([disabled]),[tabindex="0"]') || [])
        .filter(element => !!element.offsetParent);
      if (controls.length) {
        const first = controls[0]!, last = controls[controls.length - 1]!;
        if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
      }
    }
    if (event.key === 'Escape') {
      if (this.state.dialog) this.setState({ dialog: null });
      else if (this.state.asset) this.setState({ asset: null });
      else if (this.state.deskMini) this.setState({ deskMini: null });
      else if (this.state.mode === 'desk' && this.state.deskFocusId) this.setState({ deskFocusId: null });
      else if (this.state.mode === 'desk') this.setState({ mode: 'writing', deskSearchOpen: false });
      else if (this.state.queueDrawerOpen) this.setState({ queueDrawerOpen: false });
      else if (this.state.inspectorDrawerOpen) this.setState({ inspectorDrawerOpen: false });
    }
    if (this.state.mode === 'desk' && /^F[1-4]$/.test(event.key) &&
        !(event.target instanceof HTMLElement && event.target.closest('input,textarea,button,[contenteditable="true"]'))) {
      event.preventDefault();
      this.setState({ deskFocusId: event.key });
    }
  };

  private currentBody(): string {
    if (this.state.mode === 'desk') return this.props.snapshots.deskDefault;
    if (this.state.mode === 'new-draft') return this.props.snapshots.writeDefault;
    if (this.state.draftId === 'd1') return this.props.snapshots.writeDefault;
    return this.props.snapshots.writeDrafts[this.state.draftId] || this.props.snapshots.writeDefault;
  }

  private overlayFixture(): string {
    if (this.state.asset) return this.props.snapshots.assets[this.state.asset] || '';
    if (this.state.dialog) return this.props.snapshots.dialogs[this.state.dialog] || '';
    return '<div id="overlay"></div>';
  }

  private getCurrentMarkup(): { body: string; overlay: string } {
    const parsed = parseFragment(this.currentBody());
    const state = this.state;
    if (state.mode === 'writing') {
      if (state.rightTab !== 'check') {
        const right = parsed.querySelector('.right');
        const tab = parseFragment(this.props.snapshots.rightTabs[state.rightTab]);
        const replacement = tab.querySelector('.right');
        if (right && replacement) right.innerHTML = replacement.innerHTML;
      }
      parsed.querySelector('.queue')?.classList.toggle('open', state.queueDrawerOpen);
      parsed.querySelector('.right')?.classList.toggle('open', state.inspectorDrawerOpen);
      parsed.querySelector('[data-act="queueOpen"]')?.setAttribute('aria-expanded', String(state.queueDrawerOpen));
      parsed.querySelector('[data-act="rightOpen"]')?.setAttribute('aria-expanded', String(state.inspectorDrawerOpen));
    } else if (state.mode === 'desk') {
      const desk = parsed.querySelector('.desk');
      desk?.classList.toggle('is-focused', Boolean(state.deskFocusId));
      const search = parsed.querySelector<HTMLElement>('#deskSearch');
      if (search) search.hidden = !state.deskSearchOpen;
      parsed.querySelector('[data-act="deskSearchToggle"]')?.setAttribute('aria-expanded', String(state.deskSearchOpen));
      if (state.deskFocusId) {
        parsed.querySelector('[data-frame="' + state.deskFocusId + '"]')?.classList.add('focused-frame');
        const focusMarkup = this.props.snapshots.deskFocus[state.deskFocusId];
        if (focusMarkup) {
          const focus = parseFragment(focusMarkup).body.firstElementChild;
          const toolbar = desk?.querySelector('.desk-tools');
          if (focus && toolbar) toolbar.insertAdjacentElement('afterend', focus);
        }
      }
      if (state.deskMini) {
        const panelMarkup = this.props.snapshots.deskPanels[state.deskMini];
        const panel = panelMarkup ? parseFragment(panelMarkup).body.firstElementChild : null;
        const current = desk?.querySelector('.desk-editor');
        if (panel && current) current.replaceWith(panel);
        else if (panel && desk) desk.append(panel);
      }
    }
    this.prepareAccessibility(parsed);
    return { body: parsed.body.innerHTML, overlay: this.overlayFixture() };
  }

  private prepareAccessibility(parsed: Document) {
    const write = parsed.querySelector('.write');
    const desk = parsed.querySelector('.desk');
    write?.setAttribute('data-testid', 'write-workspace');
    desk?.setAttribute('data-testid', 'desk-workspace');
    desk?.setAttribute('aria-label', '沉浸材料桌');
    parsed.querySelector('.queue')?.setAttribute('data-testid', 'draft-queue');
    parsed.querySelector('.queue')?.setAttribute('aria-label', '稿件队列');
    parsed.querySelector('.editor')?.setAttribute('data-testid', 'draft-editor');
    parsed.querySelector('.editor')?.setAttribute('aria-label', '稿件编辑区');
    parsed.querySelector('.right')?.setAttribute('data-testid', 'draft-inspector');
    parsed.querySelector('.right')?.setAttribute('aria-label', '稿件检查器');
    parsed.querySelector('#edscroll')?.setAttribute('data-testid', 'editor-scroll');
    parsed.querySelector('#qlist')?.setAttribute('data-testid', 'queue-list');
    parsed.querySelector('#vp')?.setAttribute('data-testid', 'canvas-viewport');
    parsed.querySelector('#world')?.setAttribute('data-testid', 'canvas-world');
    parsed.querySelector('#vpgrid')?.setAttribute('data-testid', 'canvas-grid');
    parsed.querySelector('.cover img')?.setAttribute('data-testid', 'cover-image');
    parsed.querySelector('.cover')?.setAttribute('data-testid', 'cover-card');
    parsed.querySelectorAll<HTMLElement>('[contenteditable="true"]').forEach((element, index) => {
      if (!element.getAttribute('aria-label')) element.setAttribute('aria-label', '正文第 ' + (index + 1) + ' 段');
    });
  }

  private onInput = (event: React.FormEvent<HTMLDivElement>) => {
    const element = event.target as HTMLElement;
    if (element.getAttribute('contenteditable') === 'true') {
      const identity = element.getAttribute('data-pid') || element.id;
      if (identity) {
        const key = 'editable:' + identity;
        this.editsByDraft[this.state.draftId] = { ...this.editsByDraft[this.state.draftId], [key]: element.textContent || '' };
      }
      return;
    }
    if (!(element instanceof HTMLTextAreaElement || element instanceof HTMLInputElement)) return;
    const key = element.getAttribute('data-paragraph') || element.getAttribute('data-pid') || element.id ||
      (element.closest('[data-pid]')?.getAttribute('data-pid') ? 'paragraph:' + element.closest('[data-pid]')!.getAttribute('data-pid') : 'field:' + element.getAttribute('name'));
    this.editsByDraft[this.state.draftId] = { ...this.editsByDraft[this.state.draftId], [key]: element.value };
  };

  private showPreviewOnly(action: string) {
    this.setState({ message: action + '：当前为隔离预览，未调用正式 API、模型、采集或发布服务。' });
  }

  dispatchAction(action: string, trigger?: HTMLElement) {
    if (trigger) this.lastFocused = trigger;
    if (action === 'openAssets') this.setState({ asset: 'persona', dialog: null });
    else if (action === 'path' || action === 'tour' || action === 'saveSample' || action === 'accPop') {
      this.setState({ dialog: action === 'saveSample' ? 'save-sample' : action === 'accPop' ? 'account-pop' : action });
    } else if (action === 'deskExit') this.setState({ mode: 'writing', deskFocusId: null, deskMini: null, deskSearchOpen: false });
  }

  private onClick = (event: React.MouseEvent<HTMLDivElement>) => {
    const raw = event.target as HTMLElement;
    const target = raw.closest<HTMLElement>('[data-act]');
    if (!target) {
      if (raw.id === 'overlay' && (this.state.asset || this.state.dialog)) this.setState({ asset: null, dialog: null });
      return;
    }
    const action = target.dataset.act || '';
    const attrs = target.dataset;
    if (action !== 'modalBack') event.preventDefault();
    switch (action) {
      case 'open':
        if (attrs.id && isDraft(attrs.id)) {
          this.storeScroll();
          this.setState({ draftId: attrs.id, rightTab: 'check', queueDrawerOpen: false, inspectorDrawerOpen: false, dialog: null });
        }
        break;
      case 'rtab':
        if (attrs.t && isRightTab(attrs.t)) this.setState({ rightTab: attrs.t, inspectorDrawerOpen: true });
        break;
      case 'rightOpen': this.setState({ inspectorDrawerOpen: true }); break;
      case 'rightClose': this.setState({ inspectorDrawerOpen: false }); break;
      case 'queueOpen': this.setState({ queueDrawerOpen: true }); break;
      case 'queueClose': this.setState({ queueDrawerOpen: false }); break;
      case 'openAssets': this.storeScroll(); this.dispatchAction('openAssets', target); break;
      case 'assetsBack':
      case 'assetsClose': this.setState({ asset: null }); break;
      case 'atab': if (attrs.t && isAsset(attrs.t)) this.setState({ asset: attrs.t }); break;
      case 'deskOpen':
        this.storeScroll(); this.lastFocused = target;
        this.setState({ mode: 'desk', deskFocusId: null, deskMini: null, deskSearchOpen: false, zoom: 1, panX: 320, panY: 161.156 });
        break;
      case 'openFrame':
        this.storeScroll(); this.lastFocused = target;
        this.setState({ mode: 'desk', deskFocusId: attrs.fid || 'F1', deskMini: null, deskSearchOpen: false });
        break;
      case 'deskExit': this.dispatchAction('deskExit', target); break;
      case 'deskSearchToggle': this.setState({ deskSearchOpen: !this.state.deskSearchOpen, deskMini: null }); break;
      case 'deskFocus': this.setState({ deskFocusId: this.state.deskFocusId === attrs.fid ? null : attrs.fid || null, deskMini: null }); break;
      case 'deskFocusExit': this.setState({ deskFocusId: null, deskMini: null }); break;
      case 'deskFrameMore': this.setState({ deskMini: 'desk-frame-editor' }); break;
      case 'deskRead': this.setState({ deskMini: 'desk-card-reader' }); break;
      case 'deskRole': this.setState({ deskMini: 'desk-card-role' }); break;
      case 'deskMemo': this.setState({ deskMini: 'desk-card-memo' }); break;
      case 'deskEditorClose': this.setState({ deskMini: null }); break;
      case 'zoom': {
        const factor = Number(attrs.z) || 1;
        this.setState({ zoom: Math.max(.35, Math.min(2.8, this.state.zoom * factor)) });
        break;
      }
      case 'fit': this.setState({ zoom: 1, panX: 320, panY: 161.156 }); break;
      case 'openDraftFromFrame':
        if (attrs.id && isDraft(attrs.id)) this.setState({ draftId: attrs.id, mode: 'writing', deskFocusId: null, deskMini: null, rightTab: 'check' });
        break;
      case 'newDraft': this.lastFocused = target; this.setState({ mode: 'new-draft' }); break;
      case 'newDraftCancel':
      case 'newDraftBack': this.setState({ mode: 'writing' }); break;
      case 'path':
      case 'tour':
      case 'saveSample':
      case 'accPop':
        this.lastFocused = target;
        this.dispatchAction(action, target);
        break;
      case 'pickExp': this.lastFocused = target; this.setState({ dialog: 'pick-experience' }); break;
      case 'modalClose':
      case 'modalBack':
      case 'popClose': this.setState({ dialog: null }); break;
      case 'goAssets': this.setState({ dialog: null, asset: 'persona' }); break;
      case 'titleTab': this.setState({ rightTab: 'title' }); break;
      case 'showChecks': this.setState({ rightTab: 'check', inspectorDrawerOpen: true }); break;
      case 'copyXhs': this.showPreviewOnly('复制到小红书（服务端必改校验不可绕过）'); break;
      case 'copyTitle': this.showPreviewOnly('复制标题'); break;
      case 'sampleSave': this.showPreviewOnly('存为样板（未生成正式审核回执）'); break;
      case 'compose': this.showPreviewOnly('多账号开稿（未发起真实模型任务）'); break;
      case 'addFrame':
      case 'addNote': this.showPreviewOnly(action === 'addFrame' ? '新建选题框' : '新建便签'); break;
      case 'cardDel': this.showPreviewOnly('移出材料（未保存空间坐标）'); break;
      case 'place': this.showPreviewOnly('放进选题框（仅预览）'); break;
      case 'view': this.showPreviewOnly('运营模块路由切换（隔离预览不导航正式模块）'); break;
      case 'navOther': this.showPreviewOnly('全局模块导航（隔离预览不导航正式模块）'); break;
      case 'resetDemo':
        this.editsByDraft = {};
        this.setState({ mode: 'writing', draftId: 'd1', rightTab: 'check', asset: null, dialog: null, message: '' });
        break;
      default: this.showPreviewOnly((target.innerText || action).trim().slice(0, 20)); break;
    }
  };

  private paintDesk() {
    if (this.state.mode !== 'desk') return;
    const world = document.getElementById('world');
    const grid = document.getElementById('vpgrid');
    if (world && !this.state.deskFocusId) world.style.transform = 'translate(' + this.state.panX + 'px, ' + this.state.panY + 'px) scale(' + this.state.zoom + ')';
    if (grid && !this.state.deskFocusId) {
      grid.style.backgroundSize = (56 * this.state.zoom) + 'px ' + (56 * this.state.zoom) + 'px, ' + (14 * this.state.zoom) + 'px ' + (14 * this.state.zoom) + 'px';
      grid.style.backgroundPosition = this.state.panX + 'px ' + this.state.panY + 'px, ' + this.state.panX + 'px ' + this.state.panY + 'px';
    }
    const zoom = document.getElementById('zoomv');
    if (zoom) zoom.textContent = Math.round(this.state.zoom * 100) + '%';
  }

  private onPointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    if (this.state.mode !== 'desk' || event.button !== 0 || this.state.deskFocusId) return;
    const target = event.target as HTMLElement;
    if (!target.closest('#vp') || target.closest('button,input,textarea,.mcard,.frame')) return;
    this.worldDrag = { x: event.clientX, y: event.clientY, px: this.state.panX, py: this.state.panY };
    event.currentTarget.setPointerCapture?.(event.pointerId);
  };

  private onPointerMove = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!this.worldDrag) return;
    const x = this.worldDrag.px + event.clientX - this.worldDrag.x;
    const y = this.worldDrag.py + event.clientY - this.worldDrag.y;
    const world = document.getElementById('world');
    if (world) world.style.transform = 'translate(' + x + 'px, ' + y + 'px) scale(' + this.state.zoom + ')';
  };

  private onPointerUp = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!this.worldDrag) return;
    const drag = this.worldDrag;
    this.worldDrag = null;
    this.setState({ panX: drag.px + event.clientX - drag.x, panY: drag.py + event.clientY - drag.y });
  };

  override render() {
    const markup = this.getCurrentMarkup();
    const parsed = parseFragment(markup.body);
    const edits = this.editsByDraft[this.state.draftId] || {};
    return <div className="creatorWorkbenchGoldMaster" data-preview-only="true">
      <div className="gold-master-content" onClick={this.onClick} onInput={this.onInput}
        onPointerDown={this.onPointerDown} onPointerMove={this.onPointerMove} onPointerUp={this.onPointerUp} onPointerCancel={this.onPointerUp}>
        {Array.from(parsed.body.children).map((element, index) => convertHtmlNode(element, 'gold-master/' + index, edits))}
      </div>
      <div onClick={this.onClick}>
        <AssetOverlayPreview html={markup.overlay} edits={edits} />
      </div>
      <SyntheticFeedback text={this.state.message} />
      <div className="preview-provenance" aria-label="隔离演示标记">ISOLATED REACT PREVIEW · NO PRODUCTION API</div>
    </div>;
  }
}
