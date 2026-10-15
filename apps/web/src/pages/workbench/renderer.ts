import type { ImagePage } from './workbenchModel';
import creationFontUrl from '../../assets/fonts/noto-sans-sc.woff2?url';

/** Target-pixel template geometry; the UI only scales this canvas, never its text. */
export const RENDERER_VERSION = 'creation-renderer.v1';
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

export async function renderPage(page: ImagePage, options: RenderOptions = {}): Promise<RenderResult> {
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
