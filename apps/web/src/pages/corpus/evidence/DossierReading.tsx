import { useId } from 'react';
import { DossierGroup, EmptySpeechNotes, TranscriptGroups } from './DossierDocumentBlocks';
import { endingGroupAttribution, readingGaps, resolveReadingSections, SECTION_LABELS } from './dossierDocumentModel';
import { materialLabel } from './dossierPresent';
import type { Dossier, DossierMaterial } from './dossierTypes';
import styles from './Dossier.module.css';
import readingStyles from './DossierDocument.module.css';

/** Works and the research dossier share the same server-owned reading groups. */
export function DossierReading({ dossier, titleRendered = false, onSource }: { dossier: Dossier; titleRendered?: boolean; onSource?: (unit: DossierMaterial) => void }) {
  const prefix = `works-reading-${useId()}`;
  const titleRefs = new Set(titleRendered ? dossier.note.titleBlockRefs : []);
  const sections = resolveReadingSections(dossier).map(section => ({ ...section,
    groups: section.groups.filter(group => group.entries.some(entry => entry.block.kind !== 'gap') && !group.entries.every(entry => titleRefs.has(entry.block.ref))),
  })).filter(section => section.groups.length > 0);
  const gaps = readingGaps(dossier);
  const imageNumbers = new Map(dossier.materials.filter(unit => unit.kind === 'image').map((unit, index) => [unit.ref, index + 1]));
  return <div className={`${styles.materials} ${readingStyles.surface}`}>
    {!dossier.note.displayTitle?.trim() && sections.length === 0 ? <p className={styles.note}>暂无可读文字。</p> : null}
    {sections.map(({ section, groups }) => <section key={section.ref} aria-label={SECTION_LABELS[section.kind]} className={styles.readingGroup}>
      <h3 className={styles.readingHeading}>{SECTION_LABELS[section.kind]}</h3>
      {section.kind === 'audio' ? <TranscriptGroups groups={groups} prefix={prefix}/> : groups.map((value, index) => <DossierGroup key={value.group.ref} value={value} prefix={prefix} previousRole={endingGroupAttribution(groups[index - 1])} previousBlock={value.entries[0]?.unit.ref === groups[index - 1]?.entries.at(-1)?.unit.ref ? groups[index - 1]?.entries.at(-1)?.block : undefined}/>)}
    </section>)}
    {gaps.length ? <section aria-label="待补内容">{gaps.map(gap => <p key={gap.key} className={styles.note}>{gap.label}：{gap.state}。{gap.message}</p>)}</section> : null}
    <EmptySpeechNotes materials={dossier.materials}/>
    {onSource ? <details className={styles.disclosure}><summary>来源</summary>{dossier.materials.map(unit => <button key={unit.ref} type="button" className={styles.sourceLink} onClick={() => onSource(unit)}>{materialLabel(unit, imageNumbers.get(unit.ref))}</button>)}</details> : null}
  </div>;
}
