import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ImagePages } from './ImagePages';
import { newDocument } from './workbenchModel';
import type { ImagePage, WorkDocument } from './workbenchModel';
import { ApiError } from '../../shared/api';

vi.mock('./renderer',async importOriginal=>({...await importOriginal<typeof import('./renderer')>(),renderPage:vi.fn(async()=>({canvas:document.createElement('canvas'),issues:[],fontFamily:'Test'}))}));
beforeEach(()=>{vi.spyOn(HTMLCanvasElement.prototype,'getContext').mockReturnValue({drawImage:vi.fn()} as unknown as CanvasRenderingContext2D);});
afterEach(()=>vi.restoreAllMocks());
const page=(kind:ImagePage['kind'],title:string):ImagePage=>({id:crypto.randomUUID(),kind,templateVersion:'creation-v1',title,subtitle:'',blocks:[{id:crypto.randomUUID(),text:'原文字'}],assetId:null,crop:{x:0,y:0,width:1,height:1},style:{palette:'cream',fontFamily:'system-cjk',width:1080,height:1440,alignment:'start'}});
function initial(){return {...newDocument('正文新标题'),pages:[page('cover','手改封面'),page('opinion','观点一'),page('steps','步骤二')]};}
function Harness({value=initial(),onCapture=()=>{}}:{value?:WorkDocument;onCapture?:(doc:WorkDocument)=>void}){
  const [doc,setDoc]=useState(value);
  return <ImagePages document={doc} workId="work" onChange={next=>{onCapture(next);setDoc(next);}} onUploadAsset={vi.fn()}/>;
}
describe('image editing author control',()=>{
  it('previews title import, lets the author confirm it, and undoes without overwriting later typing',async()=>{
    render(<Harness/>);
    fireEvent.click(screen.getByRole('button',{name:'预览带入正文标题'}));
    expect(screen.getByLabelText('图页标题')).toHaveValue('手改封面');
    fireEvent.click(screen.getByRole('button',{name:'确认带入标题'})); expect(screen.getByLabelText('图页标题')).toHaveValue('正文新标题');
    fireEvent.click(screen.getByRole('button',{name:'撤销标题带入'})); expect(screen.getByLabelText('图页标题')).toHaveValue('手改封面');
    fireEvent.click(screen.getByRole('button',{name:'预览带入正文标题'}));fireEvent.click(screen.getByRole('button',{name:'确认带入标题'}));
    fireEvent.change(screen.getByLabelText('图页标题'),{target:{value:'新的手改'}});
    expect(screen.getByRole('button',{name:'撤销标题带入'})).toBeDisabled();
    await act(async()=>{});
  });
  it('reorders and restores deleted pages without changing page/block identities',async()=>{
    const doc=initial(), changes:WorkDocument[]=[];render(<Harness value={doc} onCapture={value=>changes.push(value)}/>);
    fireEvent.click(screen.getByRole('button',{name:/03 步骤/}));fireEvent.click(screen.getByRole('button',{name:'上移'}));
    expect(changes.at(-1)?.pages.map(item=>item.id)).toEqual([doc.pages[0]?.id,doc.pages[2]?.id,doc.pages[1]?.id]);
    fireEvent.click(screen.getByRole('button',{name:'删除此页'}));fireEvent.click(screen.getByRole('button',{name:'恢复删除页'}));
    expect(changes.at(-1)?.pages[1]).toEqual(doc.pages[2]); await act(async()=>{});
  });
  it('keeps manual image-page changes protected when a linked body paragraph changes',async()=>{
    const doc=initial(), paragraph={id:crypto.randomUUID(),text:'原文字'},canvasId=crypto.randomUUID();
    doc.paragraphs=[paragraph]; doc.pages[1]!.blocks[0]!.text='手改保留';
    doc.uses=[{id:crypto.randomUUID(),canvasId,expressionRevision:1,excerptId:null,target:{kind:'paragraph',paragraphId:paragraph.id},placedTextHash:'a'.repeat(64),state:'current',contentOrigin:'expression',sourceSpan:null},
      {id:crypto.randomUUID(),canvasId,expressionRevision:1,excerptId:null,target:{kind:'pageBlock',pageId:doc.pages[1]!.id,blockId:doc.pages[1]!.blocks[0]!.id},placedTextHash:'b'.repeat(64),state:'modified',contentOrigin:'expression',sourceSpan:null}];
    const onChange=vi.fn(),onUploadAsset=vi.fn();
    const view=render(<ImagePages document={doc} workId="work" onChange={onChange} onUploadAsset={onUploadAsset}/>);
    fireEvent.click(screen.getByRole('button',{name:/02 观点/}));
    view.rerender(<ImagePages document={{...doc,paragraphs:[{...paragraph,text:'正文新文字'}]}} workId="work" onChange={onChange} onUploadAsset={onUploadAsset}/>);
    await waitFor(()=>expect(screen.getByText('此处已手动修改，保留当前文字。')).toBeVisible());
    expect(screen.getByRole('button',{name:'带入这一段'})).toBeDisabled();expect(screen.getByLabelText('文案 1')).toHaveValue('手改保留');
  });
  it('recovers body-to-page update proposals after reopening from persisted use hashes',async()=>{
    const doc=initial(), paragraph={id:crypto.randomUUID(),text:'正文后来修改的文字'},canvasId=crypto.randomUUID();
    const original='原文字';const digest=await crypto.subtle.digest('SHA-256',new TextEncoder().encode(original));
    const hash=Array.from(new Uint8Array(digest),byte=>byte.toString(16).padStart(2,'0')).join('');
    doc.paragraphs=[paragraph];
    doc.uses=[{id:crypto.randomUUID(),canvasId,expressionRevision:1,excerptId:null,target:{kind:'paragraph',paragraphId:paragraph.id},placedTextHash:hash,state:'modified',contentOrigin:'expression',sourceSpan:null},
      {id:crypto.randomUUID(),canvasId,expressionRevision:1,excerptId:null,target:{kind:'pageBlock',pageId:doc.pages[1]!.id,blockId:doc.pages[1]!.blocks[0]!.id},placedTextHash:hash,state:'current',contentOrigin:'expression',sourceSpan:null}];
    const capture=vi.fn();render(<Harness value={doc} onCapture={capture}/>);
    await waitFor(()=>expect(screen.getByRole('button',{name:/02 观点.*正文已更新/})).toBeVisible());
    fireEvent.click(screen.getByRole('button',{name:/02 观点/}));
    expect(screen.getByRole('button',{name:'带入这一段'})).toBeEnabled();
    fireEvent.click(screen.getByRole('button',{name:'带入这一段'}));
    expect(capture.mock.calls.at(-1)?.[0].pages[1].blocks[0].text).toBe(paragraph.text);
    expect(capture.mock.calls.at(-1)?.[0].pages[2]).toEqual(doc.pages[2]);
    await act(async()=>{});
  });
  it('retains the exact original File and exposes explicit upload reconciliation after an unknown outcome',async()=>{
    const doc=initial(),onUpload=vi.fn().mockRejectedValueOnce(new ApiError({kind:'network'})).mockResolvedValue({id:crypto.randomUUID(),workId:'work',sha256:'a'.repeat(64),byteSize:5,mimeType:'image/png',width:400,height:300});
    render(<ImagePages document={doc} workId="work" onChange={vi.fn()} onUploadAsset={onUpload}/>);
    const file=new File(['synthetic'],'own.png',{type:'image/png'});
    fireEvent.change(screen.getByLabelText('自有图片'),{target:{files:[file]}});
    await waitFor(()=>expect(screen.getByRole('button',{name:'核实并重试原图片上传'})).toBeEnabled());
    expect(screen.getByLabelText('自有图片')).toBeDisabled();
    fireEvent.click(screen.getByRole('button',{name:'核实并重试原图片上传'}));
    await waitFor(()=>expect(screen.getByText('自有图片已上传；保存作品后保留图片关联')).toBeVisible());
    expect(onUpload).toHaveBeenCalledTimes(2);expect(onUpload.mock.calls[0]?.[0]).toBe(file);expect(onUpload.mock.calls[1]?.[0]).toBe(file);
  });
});
