import type { ImagePage } from './workbenchModel';
import creationFontUrl from '../../assets/fonts/noto-sans-sc.woff2?url';

/** Target-pixel template geometry; the UI only scales this canvas, never its text. */
export const RENDERER_VERSION = 'creation-renderer.v3';
export const FONT_PROFILE = 'noto-sans-sc.sha256:ed05125987c35ee57be16be7242568f1d516fbf497bc0ef4161499ca9e8d8fce';
export const PAGE_WIDTH = 1080;
export const PAGE_HEIGHT = 1440;
const FONT_FAMILY = 'Linggan Creation Sans';
let fontLoading: Promise<string> | undefined;
// These are the approved creation-v1 artwork palettes, not interface theme tokens.
const PALETTES = {
  cream: ['#f6f1e7', '#21261f', '#9a512f'],
  sage: ['#e9eee5', '#26362a', '#496447'],
  ink: ['#202821', '#f6f1e7', '#d9b98c'],
} as const;

export type RenderFailureCode = 'render_failure' | 'font_unavailable' | 'asset_unavailable' | 'overflow';
export class RenderError extends Error {
  constructor(readonly code: RenderFailureCode, message: string) { super(message); this.name = 'RenderError'; }
}
export interface RenderIssue { field: string; message: string }
export interface RenderResult { canvas: HTMLCanvasElement; issues: RenderIssue[]; fontFamily: string }
export interface RenderOptions { canvas?: HTMLCanvasElement; assetUrl?: string; fontFamily?: string; signal?: AbortSignal }

export function wrapText(context: Pick<CanvasRenderingContext2D, 'measureText'>, text: string, width: number): string[] {
  const lines: string[] = [];
  // Grapheme boundaries keep emoji and combining characters together.
  const segmenter = new Intl.Segmenter('zh', { granularity: 'grapheme' });
  for (const paragraph of text.split('\n')) {
    let line = '';
    for (const { segment } of segmenter.segment(paragraph)) {
      if (line && context.measureText(line + segment).width > width) { lines.push(line); line = segment; }
      else line += segment;
    }
    lines.push(line);
  }
  return lines;
}

/** V3 automatic line breaks obey Chinese opening/closing punctuation rules.
 * Explicit paragraphs and every grapheme remain intact. An unbreakable run wider
 * than the available width stays intact so the renderer exposes overflow. */
export function wrapV3Text(context: Pick<CanvasRenderingContext2D, 'measureText'>, text: string, width: number): string[] {
  const opening = /^[（(［[｛{〈《「『【〔〖〘〚‘“]/u;
  const closing = /^[）)］\]｝}〉》」』】〕〗〙〛’”。，、；：！？!?.,;:%％‰…]/u;
  const segmenter = new Intl.Segmenter('zh', {granularity:'grapheme'});
  const lines: string[] = [];
  for (const paragraph of text.split('\n')) {
    const chars=Array.from(segmenter.segment(paragraph),part=>part.segment);
    if (!chars.length) { lines.push(''); continue; }
    const legal = (end: number) => {
      if (end===chars.length) return true;
      let before=end-1,after=end;
      while (before>=0 && /^\s+$/u.test(chars[before]!)) before--;
      while (after<chars.length && /^\s+$/u.test(chars[after]!)) after++;
      return !opening.test(chars[before]??'') && !closing.test(chars[after]??'');
    };
    let start=0;
    while (start<chars.length) {
      let end=start;
      while (end<chars.length && context.measureText(chars.slice(start,end+1).join('')).width<=width) end++;
      end=Math.max(start+1,end);
      if (end<chars.length && !legal(end)) {
        let earlier=end-1;
        while (earlier>start && !legal(earlier)) earlier--;
        if (earlier>start) end=earlier;
        else while (end<chars.length && !legal(end)) end++;
      }
      lines.push(chars.slice(start,end).join(''));start=end;
    }
  }
  return lines;
}

export async function resolveFont(signal?: AbortSignal): Promise<string> {
  if (!document.fonts || typeof FontFace === 'undefined') throw new RenderError('font_unavailable', '中文字体尚未就绪，请稍后重试');
  fontLoading ??= (async () => {
    try {
      const face = new FontFace(FONT_FAMILY, `url("${creationFontUrl}")`, { weight: '100 900' });
      await face.load();
      document.fonts.add(face);
      await document.fonts.load(`700 82px "${FONT_FAMILY}"`, '中文标点，。');
      await document.fonts.ready;
      if (!document.fonts.check(`700 82px "${FONT_FAMILY}"`, '中文标点，。')) throw new Error('font check');
      return FONT_FAMILY;
    } catch {
      fontLoading = undefined;
      throw new RenderError('font_unavailable', '本地中文字体未能加载，请重新打开预览或重试');
    }
  })();
  const family = await fontLoading;
  signal?.throwIfAborted();
  return family;
}

async function renderLegacyPage(page: ImagePage, options: RenderOptions = {}): Promise<RenderResult> {
  const fontFamily = options.fontFamily ?? await resolveFont(options.signal);
  options.signal?.throwIfAborted();
  const canvas = options.canvas ?? document.createElement('canvas');
  canvas.width = PAGE_WIDTH; canvas.height = PAGE_HEIGHT;
  const context = canvas.getContext('2d');
  if (!context) throw new RenderError('render_failure', '无法建立图页画布，请重新打开预览');
  const [background, ink, accent] = PALETTES[page.style.palette];
  context.fillStyle = background; context.fillRect(0, 0, PAGE_WIDTH, PAGE_HEIGHT);
  const issues: RenderIssue[] = [];
  let y = 110;
  const alignment = page.style.alignment ?? 'start';
  const text = (value: string, field: string, size: number, weight: number, lineHeight: number,
    color: string = ink, x = 98, width = 884) => {
    context.font = `${weight} ${size}px "${fontFamily}"`;
    context.fillStyle = color;
    const lines = wrapText(context, value, width);
    const overflowing = y + (lines.length - 1) * lineHeight + size > 1300;
    if (overflowing || lines.some(line => context.measureText(line).width > width)) {
      issues.push({ field, message: '文字超出画幅，请缩短文案或预览拆页' });
    }
    context.textAlign = alignment === 'center' ? 'center' : alignment === 'end' ? 'right' : 'left';
    const drawX = alignment === 'center' ? x + width / 2 : alignment === 'end' ? x + width : x;
    // Overflow is always exposed and blocks export; font size is never changed to conceal it.
    for (const line of lines) { context.fillText(line, drawX, y + size); y += lineHeight; }
  };
  let drawAsset: (() => void) | undefined;
  if (page.assetId) {
    if (!options.assetUrl) throw new RenderError('asset_unavailable', '本页图片尚未取得，请重新选择图片');
    const image = new Image();
    image.src = options.assetUrl;
    try { await image.decode(); } catch { throw new RenderError('asset_unavailable', '图片加载失败，请重试或替换图片'); }
    options.signal?.throwIfAborted();
    const crop = page.crop;
    if (![crop.x, crop.y, crop.width, crop.height].every(Number.isFinite) || crop.width <= 0 || crop.height <= 0 ||
      crop.x < 0 || crop.y < 0 || crop.x + crop.width > 1.000001 || crop.y + crop.height > 1.000001) {
      throw new RenderError('asset_unavailable', '裁切区域无效，请重新调整');
    }
    let sx = crop.x * image.naturalWidth, sy = crop.y * image.naturalHeight;
    let sw = crop.width * image.naturalWidth, sh = crop.height * image.naturalHeight;
    const ratio = 884 / 510;
    if (sw / sh > ratio) { const next = sh * ratio; sx += (sw - next) / 2; sw = next; }
    else { const next = sw / ratio; sy += (sh - next) / 2; sh = next; }
    drawAsset = () => {
      const top = page.templateVersion === 'creation-v1-editorial' ? y : 92;
      context.drawImage(image, sx, sy, sw, sh, 98, top, 884, 510);
      y = top + 554;
      if (y > 1300) issues.push({field: 'image', message: '标题与图片超出画幅，请缩短标题或移除图片'});
    };
  }
  if (page.templateVersion !== 'creation-v1-editorial') drawAsset?.();
  text(page.title, 'title', page.kind === 'cover' ? 82 : 62, 700, page.kind === 'cover' ? 108 : 84);
  y += 32;
  if (page.subtitle) { text(page.subtitle, 'subtitle', 34, 400, 52, accent); y += 34; }
  if (page.templateVersion === 'creation-v1-editorial') drawAsset?.();
  for (const [index, block] of page.blocks.entries()) {
    if (!block.text.trim()) continue;
    context.textAlign = 'left';
    if (page.kind === 'steps') {
      context.font = `700 40px "${fontFamily}"`; context.fillStyle = accent;
      context.fillText(String(index + 1).padStart(2, '0'), 98, y + 44);
      text(block.text, block.id, 38, 400, 62, ink, 190, 792); y += 44;
    } else if (page.kind === 'voice' && index === 0) {
      context.font = `700 96px "${fontFamily}"`; context.fillStyle = accent;
      context.fillText('“', 98, y + 84); y += 90;
      text(block.text, block.id, 46, 400, 76, ink, 132, 816); y += 48;
    } else if (page.kind === 'opinion') {
      context.fillStyle = accent; context.fillRect(98, y + 14, 8, 32);
      text(block.text, block.id, 40, 400, 66, ink, 132, 850); y += 48;
    } else { text(block.text, block.id, 36, 400, 60); y += 36; }
  }
  if (!page.title.trim() && !page.blocks.some(block => block.text.trim())) issues.push({ field: 'page', message: '图页尚未填写' });
  return { canvas, issues, fontFamily };
}

// Existing template IDs keep their original geometry. A saved page opts into V3 explicitly.
export async function renderPage(page: ImagePage, options: RenderOptions = {}): Promise<RenderResult> {
  if (page.templateVersion === 'creation-v1' || page.templateVersion === 'creation-v1-editorial') return renderLegacyPage(page, options);
  if (!['creation-v3-type','creation-v3-split','creation-v3-list'].includes(page.templateVersion)) throw new RenderError('render_failure', '当前图页排版版本不可用，请重新选择排版');
  return renderV3Page(page, options);
}

async function renderV3Page(page: ImagePage, options: RenderOptions): Promise<RenderResult> {
  const fontFamily = options.fontFamily ?? await resolveFont(options.signal);
  options.signal?.throwIfAborted();
  const canvas = options.canvas ?? document.createElement('canvas');
  canvas.width = PAGE_WIDTH; canvas.height = PAGE_HEIGHT;
  const context = canvas.getContext('2d');
  if (!context) throw new RenderError('render_failure', '无法建立图页画布，请重新打开预览');
  const colors = {
    cream: {paper:'#f4efe5', ink:'#0b0f12', accent:'#a64e30', muted:'#6b6961', panel:'#e7dfcf', onAccent:'#ffffff'},
    sage: {paper:'#f4efe5', ink:'#0b0f12', accent:'#137557', muted:'#5f6a63', panel:'#dce6d8', onAccent:'#ffffff'},
    ink: {paper:'#f4efe5', ink:'#0b0f12', accent:'#0b0f12', muted:'#5f6a71', panel:'#e5e4df', onAccent:'#ffffff'},
  }[page.style.palette];
  const {paper,ink,accent,muted,panel,onAccent} = colors;
  const isSplit = page.templateVersion === 'creation-v3-split', isList = page.templateVersion === 'creation-v3-list';
  const issues: RenderIssue[] = [];
  const rect = (x:number,y:number,width:number,height:number,color:string) => { context.fillStyle=color; context.fillRect(x,y,width,height); };
  const label = (value:string,x:number,y:number,size:number,color:string,weight=400) => {
    context.font=`${weight} ${size}px "${fontFamily}"`; context.fillStyle=color; context.textAlign='left'; context.textBaseline='top'; context.fillText(value,x,y);
  };
  const block = (value:string,field:string,x:number,y:number,size:number,width:number,lineHeight:number,bottom:number,color=ink,weight=400) => {
    context.font=`${weight} ${size}px "${fontFamily}"`;
    const lines=wrapV3Text(context,value,width);
    if (y + (lines.length-1)*lineHeight + size > bottom || lines.some(line => context.measureText(line).width > width)) issues.push({field,message:'文字超出画幅，请缩短文案或预览拆页'});
    context.fillStyle=color; context.textBaseline='top';
    context.textAlign=page.style.alignment === 'center' ? 'center' : page.style.alignment === 'end' ? 'right' : 'left';
    const drawX=page.style.alignment === 'center' ? x+width/2 : page.style.alignment === 'end' ? x+width : x;
    for (const [index,line] of lines.entries()) context.fillText(line,drawX,y+index*lineHeight);
    return lines.length*lineHeight;
  };
  rect(0,0,PAGE_WIDTH,PAGE_HEIGHT,paper);
  let image: HTMLImageElement | undefined;
  if (page.assetId) {
    if (!options.assetUrl) throw new RenderError('asset_unavailable','本页图片尚未取得，请重新选择图片');
    image=new Image(); image.src=options.assetUrl;
    try { await image.decode(); } catch { throw new RenderError('asset_unavailable','图片加载失败，请重试或替换图片'); }
    options.signal?.throwIfAborted();
    const {x,y,width,height}=page.crop;
    if (![x,y,width,height].every(Number.isFinite) || width <= 0 || height <= 0 || x < 0 || y < 0 || x+width > 1.000001 || y+height > 1.000001) throw new RenderError('asset_unavailable','裁切区域无效，请重新调整');
  }
  const drawImage = (y:number) => {
    if (!image) return;
    let sx=page.crop.x*image.naturalWidth, sy=page.crop.y*image.naturalHeight;
    let sw=page.crop.width*image.naturalWidth, sh=page.crop.height*image.naturalHeight;
    const ratio=920/420;
    if (sw/sh > ratio) { const next=sh*ratio; sx+=(sw-next)/2; sw=next; }
    else { const next=sw/ratio; sy+=(sh-next)/2; sh=next; }
    context.drawImage(image,sx,sy,sw,sh,80,y,920,420);
  };
  const hasBlocks=page.blocks.some(item => item.text.trim());
  if (page.kind === 'cover') {
    if (isSplit) { rect(0,0,1080,image ? 710 : 1000,accent); rect(80,192,72,8,onAccent); }
    else if (isList) rect(0,0,1080,40,ink);
    else rect(80,96,16,48,accent);
    drawImage(250);
    const titleTop=image ? 752 : 300, titleSize=image ? 82 : 104, titleLine=image ? 112 : 156;
    const titleHeight=block(page.title,'title',80,titleTop,titleSize,920,titleLine,image ? 1090 : 944,isSplit && !image ? onAccent : ink,600);
    let contentTop=image ? titleTop+titleHeight+34 : isSplit ? 1090 : hasBlocks ? Math.max(984,titleTop+titleHeight+40) : 1080;
    if (isList && !image) { rect(80,984,920,280,panel); rect(80,984,12,280,accent); contentTop=1038; }
    else if (!image && !isSplit) rect(80,hasBlocks ? 936 : 1016,100,8,accent);
    if (page.subtitle) contentTop+=block(page.subtitle,'subtitle',isList && !image ? 128 : 80,contentTop,40,isList && !image ? 824 : 920,64,1240,ink)+24;
    for (const item of page.blocks) if (item.text.trim()) contentTop+=block(item.text,item.id,80,contentTop,36,920,56,1256,ink)+20;
  } else {
    if (isSplit) rect(0,0,1080,192,accent);
    rect(80,isSplit ? 88 : 104,72,8,isSplit ? onAccent : accent);
    if (isList) rect(0,0,1080,32,ink);
    let y=image ? 724 : 304;
    drawImage(248);
    y+=block(page.title,'title',80,y,76,920,112,1100,ink,600);
    rect(80,y+40,100,6,accent); y+=120;
    if (page.subtitle) { y+=block(page.subtitle,'subtitle',80,y,34,920,56,1240,muted)+28; }
    if (isList) rect(64,y-20,952,Math.max(0,1264-(y-20)),panel);
    for (const [index,item] of page.blocks.entries()) {
      if (!item.text.trim()) continue;
      if (page.kind === 'steps') { label(String(index+1).padStart(2,'0'),80,y+6,40,accent,700); y+=block(item.text,item.id,184,y,44,816,76,1240)+40; }
      else if (page.kind === 'voice') { if (index===0) { label('“',80,y-10,100,accent,700); y+=80; } y+=block(item.text,item.id,104,y,48,872,84,1240)+32; }
      else { y+=block(item.text,item.id,80,y,48,920,84,1240)+40; }
    }
  }
  rect(80,1300,920,2,muted);
  if (!page.title.trim() && !hasBlocks) issues.push({field:'page',message:'图页尚未填写'});
  return {canvas,issues,fontFamily};
}

export function pngBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) => canvas.toBlob(blob => blob ? resolve(blob) :
    reject(new RenderError('render_failure', 'PNG 生成失败，请重试')), 'image/png'));
}

/** Splits only at measured line boundaries, and refuses a title that cannot fit. */
export async function previewSplit(page: ImagePage, options: RenderOptions = {}): Promise<ImagePage[]> {
  const whole = await renderPage(page, options);
  if (!whole.issues.length) return [page];
  options = { ...options, fontFamily: whole.fontFamily };
  const empty = { ...page, blocks: [] };
  if ((await renderPage(empty, options)).issues.some(issue => issue.field !== 'page')) throw new RenderError('overflow', '标题或副标题过长，请先缩短再拆页');
  const context = whole.canvas.getContext('2d');
  if (!context) throw new RenderError('render_failure', '无法测量图页');
  const pages: ImagePage[] = [];
  let current: ImagePage = { ...page, blocks: [] };
  for (const block of page.blocks) {
    let remaining = block.text;
    let first = true;
    while (remaining.length) {
      const candidate = { ...current, blocks: [...current.blocks, { ...block, id: first ? block.id : crypto.randomUUID(), text: remaining }] };
      if (!(await renderPage(candidate, options)).issues.length) { current = candidate; break; }
      const chars = Array.from(new Intl.Segmenter('zh', { granularity: 'grapheme' }).segment(remaining), item => item.segment);
      let low = 0, high = chars.length;
      while (low < high) {
        const middle = Math.ceil((low + high) / 2);
        const test = { ...candidate, blocks: candidate.blocks.map((item, i) => i === candidate.blocks.length - 1 ? { ...item, text: chars.slice(0, middle).join('') } : item) };
        if ((await renderPage(test, options)).issues.length) high = middle - 1; else low = middle;
      }
      if (!low && !current.blocks.length) throw new RenderError('overflow', '当前标题与图片占用过多，请调整版式后再拆页');
      if (low) {
        current = { ...candidate, blocks: [...current.blocks, { ...block, id: first ? block.id : crypto.randomUUID(), text: chars.slice(0, low).join('') }] };
        remaining = chars.slice(low).join(''); first = false;
      }
      pages.push(current);
      current = { ...page, id: crypto.randomUUID(), kind: page.kind === 'cover' ? 'opinion' : page.kind, blocks: [] };
    }
    if (!block.text) current = { ...current, blocks: [...current.blocks, block] };
  }
  if (current.blocks.length || !pages.length) pages.push(current);
  return pages;
}
