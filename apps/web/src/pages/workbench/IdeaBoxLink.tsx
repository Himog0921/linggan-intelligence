import { ActionButton } from '../../shared/ActionButton';
import { SelectField } from '../../shared/SelectField';
import type { IdeaBoxSummary, WorkDocument } from './workbenchModel';

/**
 * An idea has its own identity. Its name and intent are edited on the anchor work.
 * It sits in the editor head as a small tag (the demo's frame chip); changing, editing or creating opens a panel under it.
 */
export function IdeaBoxLink({ document, ideas, state, edit, openAnchor, onOpenDesk, name, onNameChange, compose }: {
  document: WorkDocument; ideas: IdeaBoxSummary[]; state: 'pending' | 'error' | 'success';
  edit: (document: WorkDocument) => void; openAnchor: (anchor: string) => void; onOpenDesk?: () => void;
  name: string; onNameChange: (name: string) => void; compose: (composing: boolean) => void;
}) {
  const linked = ideas.find(idea => idea.ideaRef === document.ideaRef);
  const ownName = document.ideaName;
  const ideaOptions = [{ value: '', label: '暂不关联' }, ...ideas.map(idea => ({ value: idea.ideaRef, label: idea.name }))];
  if (document.ideaRef) return <div className="idea-link">
    <small>选题框</small>
    <strong>{ownName ?? linked?.name ?? (state === 'error' ? '读取失败' : '读取中…')}</strong>
    <span className="aux">{document.canvas.length} 张材料</span>
    {onOpenDesk ? <button type="button" className="idea-desk" aria-label="去材料桌" onClick={onOpenDesk}>↗</button> : null}
    {ownName === null && linked ? <button type="button" className="btn quiet sub" onClick={() => openAnchor(linked.anchorWorkId)}>打开选题框</button> : null}
    {ownName === null && state === 'success' ? <details><summary>更换关联</summary><div className="idea-panel">
      <label>关联已有选题框<SelectField label="更换选题框" value={document.ideaRef} options={ideaOptions} onChange={value => edit({ ...document, ideaRef: value || null })} /></label>
    </div></details> : null}
    {ownName !== null ? <details><summary>编辑选题意图</summary><div className="idea-panel">
      <label>选题名称<input className="input" aria-label="选题框名称" value={ownName} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={event => edit({ ...document, ideaName: event.target.value })} maxLength={200} /></label>
      <label>这篇想说什么<textarea className="input" aria-label="选题意图" value={document.angle} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={event => edit({ ...document, angle: event.target.value })} /></label>
    </div></details> : null}
  </div>;
  return <div className="idea-link"><small>尚未关联选题框</small><details><summary>补一个选题框</summary><div className="idea-panel">
    <label>新选题名称<input className="input" aria-label="新选题框名称" value={name} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={event => onNameChange(event.target.value)} maxLength={200} /></label>
    <ActionButton disabled={!name.trim()} onClick={() => { edit({ ...document, ideaRef: crypto.randomUUID(), ideaName: name.trim() }); onNameChange(''); }}>建立并关联</ActionButton>
    {state === 'error' ? <p role="alert">选题框读取失败，请稍后重试。</p> : state === 'pending' ? <p>正在读取已有选题框…</p> : ideas.length ? <label>关联已有选题框<SelectField label="关联已有选题框" value="" options={[{ value: '', label: '选择选题' }, ...ideas.map(idea => ({ value: idea.ideaRef, label: idea.name }))]} onChange={value => { if (value) edit({ ...document, ideaRef: value, ideaName: null }); }} /></label> : <p>还没有其他选题框。</p>}
    <small>输入名称后须点击建立；选定后随稿件保存，每个选题最多关联三个账号。</small>
  </div></details></div>;
}
