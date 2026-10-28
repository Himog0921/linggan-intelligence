import type { Ref } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import type { WorkDocument } from '../workbenchModel';
import type { BridgeState, Sentence } from './assetsModel';
import { bridgeFromSentence, bridgeStateText, groupLabel, nextSentence, withBridge } from './assetsText';
import styles from './Assets.module.css';

/**
 * The bridge slot of one draft. It says which of the five states the account is in and, when a
 * sentence may be used, lets the writer swap, rewrite, move or skip it. Sentences come from the
 * library; nothing here writes one for the writer.
 */
export function BridgeSlot({ state, document, usable, disabled, textRef, onChange, onSaveBack }: {
  state: BridgeState | null; document: WorkDocument; usable: readonly Sentence[] | null; disabled: boolean;
  textRef: Ref<HTMLTextAreaElement>; onChange: (next: WorkDocument) => void;
  onSaveBack: (sentence: { group: 'parent' | 'coach'; text: string }) => void;
}) {
  const bridge = document.bridge;
  const used = bridge?.sentence ?? null;
  const library = used ? usable?.find(item => item.code === used.sentenceCode) ?? null : null;
  const skipped = bridge?.skip === true;
  const at = used ? usable?.findIndex(item => item.code === used.sentenceCode) ?? -1 : -1;
  const position = at >= 0 ? at + 1 : 0;
  // Another usable sentence exists unless the only one is the one already in the draft.
  const alternative = !!usable && (usable.length > 1 || (usable.length === 1 && at < 0));
  return <section className={styles.slot} aria-label="衔接位">
    <h3>衔接位</h3>
    <p>{state ? bridgeStateText[state] : '正在读取账号身份…'}</p>
    {skipped ? <ActionButton variant="quiet" disabled={disabled} onClick={() => onChange(withBridge(document, { skip: false }))}>恢复衔接</ActionButton> : null}
    {state === 'ready' && !skipped ? <>
      {used ? <>
        <label>{groupLabel[used.group]} · {used.sentenceCode}{used.edited ? ' · 本篇已改写' : ''}
          <textarea ref={textRef} aria-label="本篇的衔接句" rows={3} value={used.text} disabled={disabled} maxLength={500}
            onChange={event => onChange(withBridge(document, { sentence: { ...used, text: event.target.value, edited: true } }))} />
        </label>
        <label>放在哪一段之后
          <select aria-label="衔接句的位置" value={bridge?.afterParagraph ?? ''} disabled={disabled}
            onChange={event => onChange(withBridge(document, { afterParagraph: event.target.value || null }))}>
            <option value="">默认：最后一段之前</option>
            {document.paragraphs.map((paragraph, index) => <option key={paragraph.id} value={paragraph.id}>第 {index + 1} 段之后</option>)}
          </select>
        </label>
        <div className={styles.actions}>
          <ActionButton variant="outline" disabled={disabled || !alternative} onClick={() => {
            const next = nextSentence(usable ?? [], used.sentenceCode);
            if (next) onChange(withBridge(document, { sentence: bridgeFromSentence(next) }));
          }}>换一句{position && usable ? ` ${position}/${usable.length}` : ''}</ActionButton>
          {used.edited && library ? <ActionButton variant="outline" disabled={disabled}
            onClick={() => onChange(withBridge(document, { sentence: bridgeFromSentence(library) }))}>用回库里的</ActionButton> : null}
          {used.edited && used.text.trim() ? <ActionButton variant="outline" disabled={disabled}
            onClick={() => onSaveBack({ group: used.group, text: used.text.trim() })}>存回衔接句库（待审）</ActionButton> : null}
          <ActionButton variant="quiet" disabled={disabled} onClick={() => onChange(withBridge(document, { skip: true }))}>这篇不植入</ActionButton>
        </div>
      </> : usable?.length === 0 ? <p>这个身份还没有审核通过的衔接句，先去资产库的“衔接句”里添加并审核。</p> : null}
    </> : null}
  </section>;
}
