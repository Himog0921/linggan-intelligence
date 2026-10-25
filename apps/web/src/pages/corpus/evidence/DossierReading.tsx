import { DossierBlocks, DossierIssues } from './DossierBlocks';
import { emptyMaterialNote, materialLabel, processingLabel } from './dossierPresent';
import type { Dossier, DossierBlock, DossierMaterial } from './dossierTypes';
import styles from './Dossier.module.css';

interface ReadingGroup { unit: DossierMaterial; blocks: DossierBlock[] }
/** Resolve the canonical reference order without copying text or reassembling source rules. */
function groups(dossier: Dossier, refs: readonly string[], hideTitle: boolean): ReadingGroup[] {
  const byRef = new Map(dossier.materials.flatMap(unit => unit.blocks.map(block => [block.ref, { unit, block }] as const)));
  const titleRefs = new Set(hideTitle ? dossier.note.titleBlockRefs : []);
  const result: ReadingGroup[] = [];
  for (const ref of refs) {
    if (titleRefs.has(ref)) continue;
    const value = byRef.get(ref);
    if (!value) continue; // The runtime decoder rejects dangling references.
    const previous = result.at(-1);
    if (previous?.unit.ref === value.unit.ref) previous.blocks.push(value.block);
    else result.push({ unit: value.unit, blocks: [value.block] });
  }
  return result;
}
export function DossierReading({ dossier, titleRendered = false, onSource }: { dossier: Dossier; titleRendered?: boolean; onSource?: (unit: DossierMaterial) => void }) {
  const main = groups(dossier, dossier.readingView.mainBlockRefs, titleRendered);
  const supplements = groups(dossier, dossier.readingView.supplementBlockRefs, titleRendered);
  const emptyUnits = dossier.materials.filter(unit => unit.blocks.length === 0 && (unit.sourceState !== 'not_applicable' || (unit.kind === 'audio' && unit.processing?.state === 'completed' && unit.processing.reason === 'no_audio_stream_observed')) && !(unit.kind === 'body' && unit.sourceState !== 'present'));
  const imageNumbers = new Map(dossier.materials.filter(unit => unit.kind === 'image').map((unit, index) => [unit.ref, index + 1]));
  const seenUnits = new Set<string>();
  const render = (items: ReadingGroup[]) => items.map(({ unit, blocks }, index) => {
    const first = !seenUnits.has(unit.ref); seenUnits.add(unit.ref);
    return <section className={styles.readingGroup} data-unit-ref={unit.ref} key={`${unit.ref}:${index}`}>
    {unit.kind !== 'body' ? <header className={styles.materialHead}><h4>{materialLabel(unit, imageNumbers.get(unit.ref))}</h4>{onSource ? <button type="button" className={styles.sourceLink} onClick={() => onSource(unit)}>查看来源</button> : null}</header> : null}
    {first && processingLabel(unit) ? <p className={styles.note}>{processingLabel(unit)}</p> : null}
    <DossierBlocks blocks={blocks} issues={unit.issues.filter(issue => (first || Boolean(issue.blockRef)) && !(issue.code === 'page_order_unknown' && unit.page === null))} />
  </section>; });
  const partial = Object.values(dossier.coverage.sources).some(source => ['partial', 'not_processed'].includes(source.state));
  const noReadableText = !dossier.note.displayTitle?.trim() && [...main, ...supplements].every(group => group.blocks.every(block => block.kind === 'gap'));
  return <div className={styles.materials}>
    {partial ? <p className={styles.note}>部分材料尚不可读。</p> : null}
    {noReadableText ? <p className={styles.note}>暂无可读文字。</p> : null}
    {main.length > 0 ? <section aria-label="主文"><h3 className={styles.readingHeading}>主文</h3>{render(main)}</section> : null}
    {supplements.length > 0 ? <section aria-label="补充材料"><h3 className={styles.readingHeading}>补充材料</h3>{render(supplements)}</section> : null}
    {emptyUnits.map(unit => <section key={unit.ref} className={styles.readingGroup} aria-label={`${materialLabel(unit, imageNumbers.get(unit.ref))}材料状态`}>
      <header className={styles.materialHead}><h4>{materialLabel(unit, imageNumbers.get(unit.ref))}</h4>{onSource ? <button type="button" className={styles.sourceLink} onClick={() => onSource(unit)}>查看来源</button> : null}</header>
      {processingLabel(unit) || !unit.issues.some(issue => issue.code === 'not_processed') ? <p className={styles.note}>{processingLabel(unit) ?? emptyMaterialNote(unit)}</p> : null}
      <DossierIssues issues={unit.issues.filter(issue => !(issue.code === 'page_order_unknown' && unit.page === null))} />
    </section>)}
  </div>;
}
