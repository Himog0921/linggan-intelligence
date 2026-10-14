/* Research dossier: full source view, bounded task material, and explicit local re-layering. */
(() => {
  'use strict';
  let current = null;
  const names = { title: '原标题', body: '正文', cover_headline: '图片大字', ocr: '图片正文', transcript: '视频转写', native_title: '平台标题', image_substantive_text: '图片正文', asr_text: '视频转写', frame_ocr_text: '视频画面文字' };
  const states = { ACCEPTED: '已整理', PARTIAL: '部分待确认', NEEDS_REVIEW: '待确认', FAILED: '处理失败', ACQUIRED: '已取得', QUEUED: '排队中', PROCESSING: '处理中', KNOWN_EMPTY: '本次未识别到文字', UNKNOWN: '尚未知晓', NOT_ENABLED: '未启用', WITHDRAWN_OR_RESTRICTED: '材料受限', RETIRED: '旧识别已停用' };
  const roles = { primary_copy: '图片大字', substantive_text: '图片正文', uncertain: '待确认', platform_watermark: '平台水印', platform_ui: '平台界面文字', low_confidence: '识别不清', incidental_scene_text: '场景文字' };
  function el(tag, text, cls) { const n = document.createElement(tag); if (text != null) n.textContent = text; if (cls) n.className = cls; return n; }
  function button(text, action) { const n = el('button', text, 'ev-button'); n.type = 'button'; n.addEventListener('click', action); return n; }
  function section(title) { const n = el('section', null, 'rd-section'); n.append(el('h3', title)); return n; }
  function disclosure(title, text) { const d = el('details'); d.append(el('summary', title), el('pre', text, 'rd-pre')); return d; }
  function safeImage(path) { try { const url = new URL(path, location.origin); return url.origin === location.origin && url.pathname.startsWith('/api/local/media/') ? url.href : null; } catch { return null; } }
  async function json(url, options = {}) {
    const response = await fetch(url, { cache: 'no-store', ...options });
    const payload = await response.json();
    if (!response.ok) throw new Error(payload.error || payload.code || `HTTP ${response.status}`);
    return payload;
  }
  function live(state) { return current === state && !state.controller.signal.aborted; }
  function errorText(error) {
    return String(error.message).includes('dossier_changed_or_scope_unavailable')
      ? '材料已发生变化，或本次图片超过处理范围。请刷新底稿后重试；本次没有写入。'
      : '暂时无法完成，请重试。已取得的原始材料仍保留。';
  }
  function download(state, kind) {
    const data = kind === 'md' ? state.payload.markdown : JSON.stringify(state.payload.dossier, null, 2);
    const url = URL.createObjectURL(new Blob([data], { type: kind === 'md' ? 'text/markdown;charset=utf-8' : 'application/json;charset=utf-8' }));
    const link = el('a'); link.href = url; link.download = `研究底稿-${state.options.work}.${kind}`; link.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
  function renderInput(state) {
    const box = section('送给研究任务的材料'); box.id='rd-task-input';
    const label = el('label', '选择任务 ');
    const select = el('select'); select.setAttribute('aria-label', '选择研究任务');
    [['creator', '创作者发现 · 单篇材料'], ['comments', '评论研究 · 作品上下文'], ['topics', '主题研究 · 分窗前材料']].forEach(([value, text]) => {
      const option = el('option', text); option.value = value; select.append(option);
    });
    label.append(select); box.append(label);
    const output = el('div'); box.append(output);
    const render = () => {
      const input = state.payload.inputs[select.value];
      const note = select.value === 'creator'
        ? '与任务共用单篇材料选择器，最多 4,000 字符。正式请求还会加入领域与提示词，并按模型预算缩短。'
        : select.value === 'comments'
          ? '这是任务共用的作品上下文，最多 20,000 字符。研究对象仍为评论，评论与父回复由原有研究通道读取。'
          : '这里保留该作品的完整可用材料；正式主题研究继续按原有预算分窗，并可能包含其他作品和评论。';
      const fragments = input.fragments || input.sources || [];
      output.replaceChildren(el('p', note, 'rd-note'), el('p', `已纳入 ${input.includedCharacterCount} 字符${input.truncated ? ' · 有材料因预算未纳入' : ' · 本层没有预算截断'}`));
      if (!fragments.length) output.append(el('p', '当前没有可用于这项研究的作品文字。'));
      fragments.forEach(f => output.append(disclosure(`${names[f.field || f.kind] || '材料文字'} · ${f.characterCount ?? Array.from(f.text || '').length} 字符`, f.text || '')));
      if (input.markdown) output.append(disclosure('查看标准 Markdown 材料', input.markdown));
      if (select.value === 'creator') output.append(button('按当前模型预览完整请求', async event => {
        const trigger=event.currentTarget;trigger.disabled=true;
        try {
          const prepared=await json(`${state.base}/creator-preview?domain=${encodeURIComponent(state.options.domain || '')}`,{signal:state.controller.signal});
          if (!live(state)) return;
          if (!prepared.available) output.append(el('p',prepared.reason==='model_not_configured'?'尚未配置模型，以上为默认材料预览。':'当前无法准备完整请求，请检查模型输入额度或材料范围。'));
          else {
            output.append(el('p',`模型输入上限 ${prepared.inputTokenLimit} tokens；输入保守估算上界 ${prepared.estimatedInputTokensUpperBound} tokens。正式任务会在启动时再次准备并冻结。`));
            output.append(disclosure('实际准备的 Markdown 材料',prepared.input.materials));
            output.append(disclosure('完整请求与来源映射',JSON.stringify(prepared,null,2)));
          }
        } catch { if(live(state))output.append(el('p','完整请求预览暂不可用，请重试。')); }
        finally {trigger.disabled=false;}
      }));
      output.append(disclosure('查看这份任务材料 JSON', JSON.stringify(input, null, 2)));
    };
    select.addEventListener('change', render); render(); return box;
  }
  function renderImage(state, media) {
    const layer = media.ocrLayering;
    const box = el('details', null, 'rd-image');
    box.append(el('summary', `图片 ${media.slotOrdinal ?? '—'} · ${states[layer?.state || media.state] || media.state} · ${layer?.layerVersion || '尚未分层'}`));
    const columns = el('div', null, 'rd-compare');
    const visual = el('div', null, 'rd-visual');
    const url = media.originalAssetUrl && safeImage(media.originalAssetUrl);
    if (url) {
      const image = el('img'); image.src = url; image.alt = `图片 ${media.slotOrdinal ?? ''} 原图对照`; image.loading = 'lazy';
      image.addEventListener('error', () => visual.replaceChildren(el('p', '本地原图暂时不可读，请在材料页查看获取状态。')));
      visual.append(image);
    } else visual.append(el('p', '本地原图尚不可用。可在“材料”页查看或补采。', 'rd-note'));
    const lines = el('div', null, 'rd-lines');
    const excludedBox=el('details');excludedBox.append(el('summary','已排除的水印、界面与不清文字'));
    const retained = new Map((layer?.retainedLines || []).map(l => [l.lineRef, l.classification]));
    const excluded = new Map((layer?.excludedLines || []).map(l => [l.lineRef, l.classification]));
    for (const line of layer?.lines || []) {
      const role = retained.get(line.lineRef) || excluded.get(line.lineRef) || 'uncertain';
      const row = button(`${roles[role] || '待确认'} · ${line.text}`, () => {
        visual.querySelector('.rd-highlight')?.remove();
        if (!url || !Array.isArray(line.bbox)) return;
        const [l,t,r,b] = line.bbox;
        if (![l,t,r,b].every(v => Number.isFinite(v) && v >= 0 && v <= 1)) return;
        const highlight = el('span', null, 'rd-highlight');
        Object.assign(highlight.style, { left: `${l*100}%`, top: `${t*100}%`, width: `${(r-l)*100}%`, height: `${(b-t)*100}%` }); visual.append(highlight);
      });
      row.className = 'rd-line'; row.dataset.role = role; row.title = '点击在原图标出位置'; (excluded.has(line.lineRef)?excludedBox:lines).append(row);
    }
    if (!lines.childElementCount) lines.append(el('p', '尚无可对照的识别行。'));
    if(excludedBox.childElementCount>1)lines.append(excludedBox);
    columns.append(visual, lines); box.append(columns);
    if(url)box.append(button('放大查看原图',()=>state.options.openImage?.(media)));
    if(media.otherProcessing)box.append(el('p',`另有处理结果：${states[media.otherProcessing.state] || media.otherProcessing.state}。当前展示所选可读版本。`,'rd-note'));
    state.imagePanels.set(media.derivativeRef,box);
    box.append(disclosure('原始识别文字（未筛选）', media.researchText || media.displayText || '未取得'));
    if (layer) box.append(disclosure('来源与分层版本', JSON.stringify({ layoutRef: layer.layoutRef, layeringRef: layer.layeringRef, layerVersion: layer.layerVersion, imageWidth:layer.imageWidth,imageHeight:layer.imageHeight,engineVersion:layer.engineVersion,selectionBasis:media.selectionBasis,slotKey: media.slotKey }, null, 2)));
    return box;
  }
  async function relayer(state, trigger) {
    trigger.disabled = true; state.status.textContent = '正在整理这篇作品已有的识别结果…';
    try {
      const result = await json(`${state.base}/relayer`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ expectedRevision: state.payload.dossier.revision, domain: state.options.domain || null }) });
      if (!live(state)) return;
      state.notice = `本次 ${result.scopeCount} 张：更新 ${result.updated} 张，已是当前版本 ${result.alreadyCurrent} 张，跳过 ${result.skipped} 张。`;
      await load(state);
    } catch (error) { if (live(state)) { state.status.textContent = errorText(error); trigger.disabled = false; } }
  }
  function render(state) {
    const { dossier } = state.payload;
    const head = section('研究底稿');
    state.imagePanels=new Map();
    head.append(el('p',`${dossier.metadata.platform} · ${dossier.metadata.authorDisplayName || '作者未知'} · 发布时间 ${dossier.metadata.publishedAt || '未知'}`,'rd-note'));
    const total = dossier.fragments.reduce((sum, f) => sum + f.end, 0);
    head.append(el('p', `已整理 ${dossier.fragments.length} 个文字片段 · ${total} 字符 · ${dossier.coverage.uncertainImages} 张图片部分待确认`));
    head.append(el('p', '底稿使用当前可读取的共享材料。历史运行继续保留其冻结输入；待确认行不进入正文。', 'rd-note'));
    const toolbar = el('div', null, 'rd-toolbar');
    const preview=button('预览 AI 输入',()=>document.getElementById('rd-task-input')?.scrollIntoView({block:'start'}));preview.classList.add('ev-button--primary');toolbar.append(preview);
    toolbar.append(button('刷新底稿', () => load(state)), button('导出 Markdown', () => download(state, 'md')), button('导出 JSON', () => download(state, 'json')));
    const plan=state.payload.relayerPlan;
    const count = plan.layoutCount;
    head.append(el('p',`本篇可用布局 ${count} 个 · 待分层 ${plan.pendingLayouts} 个 · 已是当前版本 ${plan.alreadyCurrent} 个 · 需先识字 ${plan.needsOcr} 个`,'rd-note'));
    const action = button(`重新分层（${count} 张）`, event => relayer(state, event.currentTarget));
    action.disabled = !count || count > 100; toolbar.append(action); head.append(toolbar);
    head.append(el('p', '重新分层只处理本篇已有的识别行，不重新识图，也不启动 AI 研究。', 'rd-note'));
    state.status = el('p', state.notice || '', 'rd-status'); state.status.setAttribute('role', 'status'); head.append(state.status);
    const coverage = section('完整度与下一步');
    const labels = { KNOWN: '已取得', KNOWN_EMPTY: '已确认没有', UNKNOWN: '未知', UNAVAILABLE: '暂不可用' };
    coverage.append(el('p', `原标题：${labels[dossier.coverage.title] || dossier.coverage.title}；正文：${labels[dossier.coverage.body] || dossier.coverage.body}；媒体处理结果：${dossier.coverage.mediaAvailable}/${dossier.coverage.mediaCount} 可读取。`));
    coverage.append(el('p', '缺少原文或原图：先在“材料”查看获取状态，再通过现有采集任务补齐。识别已取得但分层不理想：使用上方“重新分层”。这份底稿只代表已经采到的材料。', 'rd-note'));
    const content = section('完整文字');
    if (!dossier.fragments.length) content.append(el('p', '暂未取得可用文字；这不等于原作品没有内容。'));
    dossier.sections.forEach(part => {
      const block=section(part.label);
      const fragments=dossier.fragments.filter(f=>part.fragmentIds.includes(f.fragmentId));
      if (!fragments.length) block.append(el('p',part.kind==='comments'?'评论在下方单独读取。':part.kind==='background'?'待确认行保留在原图对照中；当前规则不猜测照片中文件的类型。':part.kind==='tags'?'尚无独立标签记录；正文中的标签保留在原文中。':'当前没有可用内容；是否原本存在尚未确认。','rd-note'));
      fragments.forEach(f=>{
        const text=disclosure(`展开全文 · ${f.end} 字符`,f.text);text.open=f.end<500;block.append(text);
        if (['cover_headline','ocr'].includes(f.field))block.append(button('对照原图',()=>{const target=state.imagePanels.get(f.sourceRef);if(target){target.open=true;target.scrollIntoView({block:'start'});}}));
        block.append(disclosure('查看来源',JSON.stringify({fragmentId:f.fragmentId,sourceRef:f.sourceRef,sourceVersion:f.sourceVersion},null,2)));
      });content.append(block);
    });
    const images = section('原图与识别对照');
    const media = dossier.media.filter(m => m.contextOnly!==true && (m.ocrLayering || ['image_ocr','video_frame_ocr'].includes(m.processorKind)));
    media.forEach(m => images.append(renderImage(state,m)));
    if (!media.length) images.append(el('p', '当前没有图片识别结果。'));
    const meta = section('互动与来源信息');
    meta.append(el('p', `点赞 ${dossier.metadata.likes.value ?? '未知'} · 评论 ${dossier.metadata.comments.value ?? '未知'} · 收藏 ${dossier.metadata.collects.value ?? '未知'}`));
    meta.append(el('p', '评论原文继续从现有评论研究入口读取；评论中的表述不会混入作品正文。', 'rd-note'));
    meta.append(disclosure('底稿版本与观察信息', JSON.stringify({ contract: dossier.contract, revision: dossier.revision, asOf: dossier.asOf, metadata: dossier.metadata }, null, 2)));
    state.panel.replaceChildren(head, coverage, renderInput(state), content, images, renderComments(state), meta);
  }
  function renderComments(state) {
    const box=section('已采评论');
    if(!state.options.domain){box.append(el('p','请选择领域后，从授权评论通道查看。'));return box;}
    const list=el('div'); const status=el('p','按已取得的来源稳定顺序读取，不代表平台展示顺序。','rd-note');
    let cursor=null, loaded=0; const rows=new Map();
    const next=button('读取前 30 条',async()=>{
      next.disabled=true;
      try {
        const params=new URLSearchParams({domain:state.options.domain});if(cursor)params.set('cursor',cursor);
        const page=await json(`${state.base}/comments?${params}`,{signal:state.controller.signal});if(!live(state))return;
        for(const item of page.items) rows.set(item.sourceRef,item);
        const nodes=new Map();list.replaceChildren();
        for(const item of rows.values()) nodes.set(item.sourceRef,disclosure(`${item.relation==='REPLY'?'回复':'主评论'} · 赞 ${item.likeCount ?? '未知'} · ${item.body?.slice(0,80) || '原文未知'}`,item.body || '尚未取得评论文字'));
        for(const item of rows.values()) {
          const d=nodes.get(item.sourceRef);const parent=rows.get(item.parentSourceRef);
          const replies=[...rows.values()].filter(r=>r.parentSourceRef===item.sourceRef).length;
          if(replies)d.querySelector('summary').append(document.createTextNode(` · ${replies} 条已采回复`));
          (parent?.relation==='ROOT'?nodes.get(item.parentSourceRef):list).append(d);
          if(item.relation==='REPLY'&&!parent)d.append(el('p','父评论尚未读到，暂不拼接其内容。','rd-note'));
        }
        loaded+=page.items.length;cursor=page.nextCursor;
        status.textContent=`已展示 ${loaded} / ${page.total} 条可读取评论；平台曾显示 ${state.payload.dossier.metadata.comments.value ?? '未知'} 条。数量相同不代表已采全。`;
        next.hidden=!cursor;next.textContent='继续读取 30 条';
      }catch{if(live(state))status.textContent='评论读取暂不可用，底稿其他部分仍可使用。';}
      finally{next.disabled=false;}
    });
    box.append(status,list,next);return box;
  }
  async function load(state) {
    if (!live(state) || state.loading) return;
    state.loading = true; state.panel.setAttribute('aria-busy', 'true');
    if (!state.payload) state.panel.replaceChildren(el('p', '正在整理当前已取得的研究材料…'));
    else state.status.textContent = '正在刷新底稿…';
    try {
      const params = new URLSearchParams(); if (state.options.domain) params.set('domain', state.options.domain);
      const payload = await json(`${state.base}?${params}`, { signal: state.controller.signal });
      if (!live(state)) return;
      state.payload = payload; render(state);
    } catch (error) {
      if (!live(state) || error.name === 'AbortError') return;
      if(state.payload && state.status)state.status.replaceChildren(el('span',errorText(error)+' 当前保留上一次可读底稿。'),button('重试刷新',()=>load(state)));
      else state.panel.replaceChildren(el('p', errorText(error)), button('重试读取底稿', () => load(state)));
    } finally { if (live(state)) { state.loading = false; state.panel.removeAttribute('aria-busy'); } }
  }
  window.LingganResearchDossier = {
    reset() { current?.controller.abort(); current = null; },
    mount(panel, options) {
      this.reset(); if (!panel) return;
      current = { panel, options, controller: new AbortController(), base: `/api/local/work-resources/${encodeURIComponent(options.work)}/research-dossier` };
      const state = current;
      panel.replaceChildren(el('p', '查看这篇作品的完整研究文字、来源与图片分层。'), button('读取研究底稿', () => load(state)));
      if (!panel.hidden) load(state);
    },
    show() { if (current && !current.payload) load(current); },
  };
})();
