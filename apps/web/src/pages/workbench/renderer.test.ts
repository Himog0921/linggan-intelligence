import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ImagePage } from './workbenchModel';
import { previewSplit, renderPage, resolveFont, wrapText } from './renderer';

const page = (kind: ImagePage['kind'] = 'cover'): ImagePage => ({id: crypto.randomUUID(), kind, templateVersion:'creation-v1', title:'作业启动', subtitle:'先迈出第一步',
  blocks:[{id:crypto.randomUUID(),text:'把任务分成小步。'}],assetId:null,crop:{x:0,y:0,width:1,height:1},style:{palette:'cream',fontFamily:'system-cjk',width:1080,height:1440,alignment:'start'}});
let context: CanvasRenderingContext2D;
beforeEach(() => {
  context = {font:'',fillStyle:'',textAlign:'left',fillRect:vi.fn(),drawImage:vi.fn(),fillText:vi.fn(),
    measureText(this: {font: string}, text: string) { const size = Number(/ (\d+)px/.exec(this.font)?.[1] ?? 40); return {width:Array.from(text).length * size} as TextMetrics; },
  } as unknown as CanvasRenderingContext2D;
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(context);
});
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });

describe('creation renderer target-pixel rules', () => {
  it('keeps emoji graphemes and explicit blank lines intact', () => {
    const measuring = {measureText: (value: string) => ({width:Array.from(new Intl.Segmenter('zh',{granularity:'grapheme'}).segment(value)).length}) as TextMetrics};
    expect(wrapText(measuring, '中👨‍👩‍👧‍👦文\n\n末', 2)).toEqual(['中👨‍👩‍👧‍👦','文','','末']);
  });
  it('renders at 1080×1440 and reports overflowing text without shrinking it', async () => {
    const input = page(); input.title = '超长标题'.repeat(150);
    const result = await renderPage(input,{fontFamily:'Test CJK'});
    expect([result.canvas.width,result.canvas.height]).toEqual([1080,1440]);
    expect(result.issues.some(issue => issue.field === 'title')).toBe(true);
    expect(context.fillText).toHaveBeenCalled();
    expect(input.title).toBe('超长标题'.repeat(150));
  });
  it('splits measured text with stable original IDs, new continuation IDs, and no text loss', async () => {
    const input = page('opinion'); input.blocks[0]!.text = '孩子可以先把书本放到桌上，再看看第一题。'.repeat(35);
    const result = await previewSplit(input,{fontFamily:'Test CJK'});
    expect(result.length).toBeGreaterThan(1);
    expect(result[0]?.id).toBe(input.id);
    expect(result.flatMap(item => item.blocks).map(block => block.text).join('')).toBe(input.blocks[0]?.text);
    expect(new Set(result.map(item=>item.id)).size).toBe(result.length);
    const ids=result.flatMap(item=>item.blocks).map(block=>block.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const output of result) expect((await renderPage(output,{fontFamily:'Test CJK'})).issues).toEqual([]);
  });
  it('does not split an overflowing heading into apparently valid output', async () => {
    const input=page(); input.title='标题'.repeat(500);
    await expect(previewSplit(input,{fontFamily:'Test CJK'})).rejects.toMatchObject({code:'overflow'});
  });
  it('fails explicitly when a required own image is unavailable', async () => {
    const input=page(); input.assetId=crypto.randomUUID();
    await expect(renderPage(input,{fontFamily:'Test CJK'})).rejects.toMatchObject({code:'asset_unavailable'});
  });
  it('waits for font readiness and rejects unavailable CJK glyphs', async () => {
    vi.stubGlobal('FontFace',class { async load() {return this;} });
    Object.defineProperty(document,'fonts',{configurable:true,value:{ready:Promise.resolve(),load:vi.fn().mockResolvedValue([]),check:()=>false}});
    await expect(resolveFont()).rejects.toMatchObject({code:'font_unavailable'});
  });
  it('loads the bundled local font before rendering and shares the same loaded face', async () => {
    const faceLoad=vi.fn().mockResolvedValue(undefined),add=vi.fn(),fontLoad=vi.fn().mockResolvedValue([]);
    const constructor=vi.fn();
    vi.stubGlobal('FontFace',class {constructor(family:string,url:string){constructor(family,url);}async load(){await faceLoad();return this;}});
    Object.defineProperty(document,'fonts',{configurable:true,value:{ready:Promise.resolve(),load:fontLoad,add,check:()=>true}});
    expect(await resolveFont()).toBe('Linggan Creation Sans');
    expect(await resolveFont()).toBe('Linggan Creation Sans');
    expect(faceLoad).toHaveBeenCalledTimes(1);expect(add).toHaveBeenCalledTimes(1);
    expect(constructor).toHaveBeenCalledWith('Linggan Creation Sans',expect.stringContaining('noto-sans-sc.woff2'));
    expect(fontLoad).toHaveBeenCalled();
  });
});
