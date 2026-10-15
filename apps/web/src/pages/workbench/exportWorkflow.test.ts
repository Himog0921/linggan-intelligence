import { describe, expect, it, vi } from 'vitest';
import type { ExportRecord, Revision } from './workbenchModel';
import { newDocument } from './workbenchModel';
import { ExportWorkflow } from './exportWorkflow';
import type { ExportTransport } from './exportWorkflow';
import { FONT_PROFILE, RenderError } from './renderer';
import { ApiError } from '../../shared/api';

const frozenRevision = ():Revision => ({id:crypto.randomUUID(),number:3,savedAt:'2026-10-15T00:00:00Z',document:{...newDocument('冻结标题'),accountRef:crypto.randomUUID(),
  pages:[{id:crypto.randomUUID(),kind:'cover',templateVersion:'creation-v1',title:'冻结封面',subtitle:'',blocks:[],assetId:null,crop:{x:0,y:0,width:1,height:1},style:{palette:'cream',fontFamily:'system-cjk',width:1080,height:1440,alignment:'start'}}]}});
function setup(){
  const revision=frozenRevision();
  const record:ExportRecord={id:crypto.randomUUID(),workId:crypto.randomUUID(),revisionId:revision.id,state:'rendering',failureCode:null,rendererVersion:'creation-renderer.v1',fontVersion:FONT_PROFILE,
    pages:revision.document.pages.map((page,index)=>({pageId:page.id,ordinal:index+1,sha256:null,byteSize:null})),packageSha256:null,packageByteSize:null};
  const complete={...record,state:'complete',packageSha256:'hash',packageByteSize:100};
  const transport:ExportTransport={freeze:vi.fn().mockResolvedValue({export:record,revision}),upload:vi.fn().mockResolvedValue(record),finalize:vi.fn().mockResolvedValue(complete),retry:vi.fn().mockResolvedValue({export:record,revision}),fail:vi.fn().mockResolvedValue({...record,state:'failed'}),lookup:vi.fn().mockResolvedValue(null),read:vi.fn().mockResolvedValue(record)};
  const render=vi.fn().mockResolvedValue(new Blob(['exact frozen png'],{type:'image/png'}));
  return {revision,record,complete,transport,render,workflow:new ExportWorkflow(transport,render)};
}
describe('immutable export workflow',()=>{
  it('waits for confirmed save and renders only the server frozen revision',async()=>{
    const s=setup(); let resolveSave:(value:Revision)=>void=()=>{};
    const save=new Promise<Revision>(resolve=>{resolveSave=resolve;});
    const run=s.workflow.start(s.record.workId,()=>save);
    await Promise.resolve(); expect(s.transport.freeze).not.toHaveBeenCalled(); expect(s.render).not.toHaveBeenCalled();
    const editable=frozenRevision(); editable.document.pages[0]!.title='编辑器里的新标题'; resolveSave(editable);
    await run;
    expect(s.transport.freeze).toHaveBeenCalledWith(s.record.workId,expect.objectContaining({expectedRevision:editable.id}));
    expect(s.render.mock.calls[0]?.[0].title).toBe('冻结封面');
    expect(s.transport.finalize).toHaveBeenCalled();
  });
  it('replays an uncertain freeze with its original request identity and payload',async()=>{
    const s=setup(); vi.mocked(s.transport.freeze).mockRejectedValueOnce(new Error('network'));
    const save=vi.fn().mockResolvedValue(s.revision);
    await expect(s.workflow.start(s.record.workId,save)).rejects.toThrow('network');
    const original=vi.mocked(s.transport.freeze).mock.calls[0];
    await s.workflow.retry();
    expect(vi.mocked(s.transport.freeze).mock.calls[1]).toEqual(original);
    expect(s.transport.lookup).toHaveBeenCalledWith(original?.[1].requestId);
    expect(vi.mocked(s.transport.lookup).mock.invocationCallOrder[0]).toBeLessThan(vi.mocked(s.transport.freeze).mock.invocationCallOrder[1]!);
    expect(save).toHaveBeenCalledTimes(1);
  });
  it('retains exact PNG bytes and finalize identity after uncertain upload/finalize outcomes',async()=>{
    const s=setup(); vi.mocked(s.transport.upload).mockRejectedValueOnce(new Error('upload network'));
    await expect(s.workflow.start(s.record.workId,async()=>s.revision)).rejects.toThrow('upload network');
    const first=vi.mocked(s.transport.upload).mock.calls[0]?.[2];
    vi.mocked(s.transport.finalize).mockRejectedValueOnce(new Error('finalize network'));
    await expect(s.workflow.retry()).rejects.toThrow('finalize network');
    const request=vi.mocked(s.transport.finalize).mock.calls[0]?.[1];
    await s.workflow.retry();
    expect(s.render).toHaveBeenCalledTimes(1);
    expect(vi.mocked(s.transport.upload).mock.calls.every(call=>call[2]===first)).toBe(true);
    expect(vi.mocked(s.transport.finalize).mock.calls[1]?.[1]).toEqual(request);
    expect(s.transport.fail).not.toHaveBeenCalled();
    expect(s.transport.read).toHaveBeenCalledWith(s.record.id);
  });
  it('records known renderer failure, resumes frozen revision, and never rolls back the editor',async()=>{
    const s=setup(); s.render.mockRejectedValueOnce(new RenderError('overflow','文字超出'));
    await expect(s.workflow.start(s.record.workId,async()=>s.revision)).rejects.toThrow('文字超出');
    expect(s.transport.fail).toHaveBeenCalledWith(s.record.id,expect.objectContaining({code:'overflow'}));
    await s.workflow.retry(); expect(s.transport.retry).toHaveBeenCalledWith(s.record.id,expect.anything());
    expect(s.workflow.frozenRevisionId).toBe(s.revision.id);
  });
  it('resumes interrupted records using server snapshot; completed exports remain immutable',async()=>{
    const s=setup(); await s.workflow.resume(s.record);
    expect(s.transport.freeze).not.toHaveBeenCalled(); expect(s.transport.retry).toHaveBeenCalled();
    vi.clearAllMocks(); expect(await s.workflow.resume(s.complete)).toBe(s.complete);
    expect(s.render).not.toHaveBeenCalled(); expect(s.transport.upload).not.toHaveBeenCalled();
  });
  it('does not rerender an old interrupted package using a different font profile',async()=>{
    const s=setup();s.record.fontVersion='old-font';
    await expect(s.workflow.resume(s.record)).rejects.toThrow('字体或绘图版本');
    expect(s.render).not.toHaveBeenCalled();expect(s.transport.upload).not.toHaveBeenCalled();
  });
  it('refuses a frozen receipt whose page order or revision differs from the snapshot',async()=>{
    const s=setup();s.record.pages[0]!.pageId=crypto.randomUUID();
    await expect(s.workflow.start(s.record.workId,async()=>s.revision)).rejects.toThrow('冻结图页回执不一致');
    expect(s.transport.upload).not.toHaveBeenCalled();
  });
  it('retains the retry request identity when a server resume outcome is uncertain',async()=>{
    const s=setup();vi.mocked(s.transport.retry).mockRejectedValueOnce(new Error('network'));
    await expect(s.workflow.resume(s.record)).rejects.toThrow('network');
    const request=vi.mocked(s.transport.retry).mock.calls[0];await s.workflow.retry();
    expect(vi.mocked(s.transport.retry).mock.calls[1]).toEqual(request);
    expect(s.transport.freeze).not.toHaveBeenCalled();
  });
  it('uses the original freeze receipt from GET without sending a second mutation',async()=>{
    const s=setup();vi.mocked(s.transport.freeze).mockRejectedValueOnce(new ApiError({kind:'network'}));
    await expect(s.workflow.start(s.record.workId,async()=>s.revision)).rejects.toBeInstanceOf(ApiError);
    vi.mocked(s.transport.lookup).mockResolvedValue({export:s.record,revision:s.revision});await s.workflow.retry();
    expect(s.transport.freeze).toHaveBeenCalledTimes(1);expect(s.render).toHaveBeenCalled();
  });
  it('keeps unknown commands recoverable and blocks starting a replacement before reconciliation',async()=>{
    const s=setup();vi.mocked(s.transport.freeze).mockRejectedValueOnce(new ApiError({kind:'network'}));
    await expect(s.workflow.start(s.record.workId,async()=>s.revision)).rejects.toBeInstanceOf(ApiError);
    expect(s.workflow.hasUnconfirmed).toBe(true);
    await expect(s.workflow.start(s.record.workId,async()=>s.revision)).rejects.toThrow('原生成请求结果尚未确认');
    vi.mocked(s.transport.lookup).mockRejectedValueOnce(new ApiError({kind:'http',status:403,code:'forbidden'}));
    await expect(s.workflow.retry()).rejects.toBeInstanceOf(ApiError);expect(s.workflow.hasUnconfirmed).toBe(true);
    expect(s.transport.freeze).toHaveBeenCalledTimes(1);
  });
  it('can generate one selected PNG without finalizing or rendering unrelated overflowing pages',async()=>{
    const s=setup();const second={...s.revision.document.pages[0]!,id:crypto.randomUUID(),kind:'opinion' as const,title:'单页'};
    s.revision.document.pages.push(second);s.record.pages.push({pageId:second.id,ordinal:2,sha256:null,byteSize:null});
    await s.workflow.start(s.record.workId,async()=>s.revision,undefined,second.id);
    expect(s.render).toHaveBeenCalledTimes(1);expect(s.render.mock.calls[0]?.[0].id).toBe(second.id);
    expect(s.transport.finalize).not.toHaveBeenCalled();
  });
});
