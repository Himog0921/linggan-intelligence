import { ActionButton } from '../../shared/ActionButton';
import type { IdeaBoxSummary, WorkDocument } from './workbenchModel';
import styles from './WorkbenchPage.module.css';

/** An idea has its own identity. Its name and intent are edited on the anchor work. */
export function IdeaBoxLink({ document, ideas, state, edit, openTopic, name, onNameChange, compose }: {
  document: WorkDocument; ideas: IdeaBoxSummary[]; state: 'pending' | 'error' | 'success';
  edit: (document: WorkDocument) => void; openTopic: (anchor: string) => void;
  name: string; onNameChange: (name: string) => void;
  compose: (composing: boolean) => void;
}) {
  const linked = ideas.find(idea => idea.ideaRef === document.ideaRef);
  const ownName = document.ideaName;
  if (document.ideaRef) return <div className={styles.ideaBar}>
    <small>选题框</small>
    <strong>{ownName ?? linked?.name ?? (state === 'error' ? '读取失败' : '读取中…')}</strong>
    {ownName === null && linked ? <ActionButton variant="quiet" onClick={() => openTopic(linked.anchorWorkId)}>打开选题框</ActionButton> : null}
    {ownName === null && state === 'success' ? <details><summary>更换关联</summary>
      <label>关联已有选题框<select aria-label="更换选题框" value={document.ideaRef} onChange={event => edit({ ...document, ideaRef: event.target.value || null })}>
        <option value="">暂不关联</option>{ideas.map(idea => <option key={idea.ideaRef} value={idea.ideaRef}>{idea.name}</option>)}
      </select></label>
    </details> : null}
    {ownName !== null ? <details><summary>编辑选题意图</summary>
      <label>选题名称<input aria-label="选题框名称" value={ownName} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={event => edit({ ...document, ideaName: event.target.value })} maxLength={200} /></label>
      <label>这篇想说什么<textarea aria-label="选题意图" value={document.angle} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={event => edit({ ...document, angle: event.target.value })} /></label>
    </details> : null}
  </div>;
  return <div className={styles.ideaBar}><small>尚未关联选题框</small><details><summary>补一个选题框</summary>
    <label>新选题名称<input aria-label="新选题框名称" value={name} onCompositionStart={() => compose(true)} onCompositionEnd={() => compose(false)} onChange={event => onNameChange(event.target.value)} maxLength={200} /></label>
    <ActionButton disabled={!name.trim()} onClick={() => { edit({ ...document, ideaRef: crypto.randomUUID(), ideaName: name.trim() }); onNameChange(''); }}>建立并关联</ActionButton>
    {state === 'error' ? <p role="alert">选题框读取失败，请稍后重试。</p> : state === 'pending' ? <p>正在读取已有选题框…</p> : ideas.length ? <label>关联已有选题框<select aria-label="关联已有选题框" value="" onChange={event => { if (event.target.value) edit({ ...document, ideaRef: event.target.value, ideaName: null }); }}>
      <option value="">选择选题</option>{ideas.map(idea => <option key={idea.ideaRef} value={idea.ideaRef}>{idea.name}</option>)}
    </select></label> : <p>还没有其他选题框。</p>}
    <small>输入名称后须点击建立；选定后随稿件保存，每个选题最多关联三个账号。</small>
  </details></div>;
}
