import { Fragment, useId, useMemo, useRef, useState } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { humanMoment } from '../../../shared/formatting';
import { DossierGroup, ReadingIssues, TranscriptGroups } from './DossierDocumentBlocks';
import { DossierExportDialog } from './DossierExportDialog';
import { DossierFocusDialog } from './DossierFocusDialog';
import { DossierIcon } from './DossierIcon';
import { DossierNavigation } from './DossierNavigation';
import { endingGroupAttribution, groupTime, readingAnchor, readingGaps, remainingReadingIssues, resolveReadingSections, SECTION_LABELS, transcriptStats, type ResolvedReadingGroup } from './dossierDocumentModel';
import { jumpReading, useDossierReadingPosition, type ReadingPosition } from './useDossierReadingPosition';
import type { DossierMaterial, DossierPayload } from './dossierTypes';
import styles from './DossierDocument.module.css';

interface Props { payload: DossierPayload; domain: string; active: boolean; busy: boolean; refreshFailed: boolean; onRefresh: () => void; onUnavailable: () => void }
function audioRuns(groups: readonly ResolvedReadingGroup[]) {
  const runs: { ref: string; groups: ResolvedReadingGroup[] }[] = [];
  for (const group of groups) {
    const ref = group.entries[0]?.unit.ref ?? group.group.ref;
    const previous = runs.at(-1);
    if (previous?.ref === ref) previous.groups.push(group);
    else runs.push({ ref, groups: [group] });
  }
  return runs;
}
function ImageGap({ pages }: { pages: readonly number[] }) { return <p className={styles.gap}>此处内容不完整（第 {pages.join('、')} 页文字待补全）</p>; }
function readableSnapshot(previous: readonly DossierMaterial[], latest: readonly DossierMaterial[]): boolean {
  const current = new Map(latest.map(unit => [unit.ref, unit]));
  return previous.every(unit => {
    if (unit.blocks.length === 0) return true;
    const next = current.get(unit.ref);
    return next !== undefined && next.sourceState !== 'restricted' && next.sourceState !== 'not_acquired';
  });
}
export function DossierDocument({ payload, domain, active, busy, refreshFailed, onRefresh, onUnavailable }: Props) {
  const { dossier } = payload;
  const id = useId(), prefix = `dossier-${id}`;
  const root = useRef<HTMLDivElement>(null), toolbar = useRef<HTMLDivElement>(null);
  const beforeModal = useRef<ReadingPosition | null>(null);
  const [exportSnapshot, setExportSnapshot] = useState<DossierPayload | null>(null);
  const [focus, setFocus] = useState<{ ref: string; title: string; groups: ResolvedReadingGroup[] } | null>(null);
  // A revision update may retain a frozen export; a revoked source may not retain its old text.
  if (exportSnapshot && !readableSnapshot(exportSnapshot.dossier.materials, dossier.materials)) setExportSnapshot(null);
  if (focus && !readableSnapshot(focus.groups.flatMap(group => group.entries.map(entry => entry.unit)), dossier.materials)) setFocus(null);
  const sections = useMemo(() => resolveReadingSections(dossier), [dossier]);
  const gaps = useMemo(() => readingGaps(dossier), [dossier]);
  const issues = useMemo(() => remainingReadingIssues(dossier, sections), [dossier, sections]);
  const hasIssues = issues.length > 0 || sections.some(section => section.groups.some(group => group.issues.length > 0));
  const hasMissing = gaps.length > 0 || issues.length > 0;
  const selection = dossier.selection;
  const memoryKey = `dossier-reading:v3:${domain}:${dossier.workRef}:${JSON.stringify([selection.mode, selection.kinds, selection.unitRefs, selection.blockRefs, selection.imagePages])}`;
  const position = useDossierReadingPosition(root, toolbar, memoryKey, active, dossier.revision);
  const navSections = sections.map(({ section }, index) => ({ ref: section.ref, label: SECTION_LABELS[section.kind], number: String(index + 1).padStart(2, '0') }));
  if (hasMissing) navSections.push({ ref: 'reading:missing', label: '缺失内容', number: String(navSections.length + 1).padStart(2, '0') });
  const times = sections.filter(section => section.section.kind === 'audio').flatMap(section => section.groups.flatMap(group => {
    const time = groupTime(group);
    return group.group.mode === 'transcript_paragraph' && time ? [{ ref: group.group.ref, startMs: time.startMs }] : [];
  }));
  const showNavigation = sections.length > 1 || times.length > 1 || sections.some(section => section.groups.length > 3);
  function jump(ref: string, first = false) {
    const element = document.getElementById(readingAnchor(prefix, ref));
    if (root.current && element && root.current.contains(element)) jumpReading(root.current, element, position.inset(), first);
  }
  function openExport() { beforeModal.current = position.capture(); position.save(); setExportSnapshot(payload); }
  function openFocus(run: { ref: string; groups: ResolvedReadingGroup[] }) { beforeModal.current = position.capture(); position.save(); setFocus({ ...run, title: dossier.note.displayTitle ?? '无标题' }); }
  function closeModal() { setExportSnapshot(null); setFocus(null); position.restore(beforeModal.current); }
  return <div ref={root} className={`${styles.surface} ${styles.root}`} aria-label="研究底稿正文" aria-busy={busy} data-dossier-document>
    <div ref={toolbar} className={styles.toolbar}>
      <div className={styles.toolbarContext}>
        <div className={styles.snapshot}><DossierIcon name="clock"/><span>数据截至</span><time dateTime={dossier.asOf}>{humanMoment(dossier.asOf)}</time><span className={styles.zone}>UTC+8</span></div>
        <span className={styles.availability} data-complete={gaps.length === 0 && !hasIssues && sections.length > 0}><DossierIcon name={gaps.length || hasIssues ? 'info' : 'check'}/>{gaps.length ? '含待补内容' : hasIssues ? '含待核对项' : sections.length ? '内容就绪' : '暂无可读文字'}</span>
        {showNavigation ? <DossierNavigation sections={navSections} times={times} current={position.current} root={root} toolbar={toolbar} onJump={jump}/> : null}
      </div>
      <div className={styles.actions}><ActionButton className={`${styles.control} ${styles.quiet}`} aria-label="刷新底稿" title="刷新底稿" disabled={busy} onClick={onRefresh}><DossierIcon name="refresh"/><span className={styles.refreshLabel}>刷新</span></ActionButton><ActionButton className={`${styles.control} ${styles.primary}`} onClick={openExport}><DossierIcon name="download"/>导出</ActionButton></div>
    </div>
    {refreshFailed ? <p className={styles.notice} role="status">刷新未完成，当前保留上一次读到的版本。<button type="button" onClick={onRefresh}>重试刷新</button></p> : null}
    <article className={styles.document} aria-label="连续阅读文档">
      {sections.map(({ section, groups }, index) => <section key={section.ref} className={styles.section} id={readingAnchor(prefix, section.ref)} data-section-ref={section.ref} data-reading-anchor={section.ref} tabIndex={-1} aria-labelledby={readingAnchor(prefix, `${section.ref}:title`)}>
        <header className={styles.sectionIndex}><span className={styles.sectionNumber}>{String(index + 1).padStart(2, '0')}</span><h2 id={readingAnchor(prefix, `${section.ref}:title`)}>{SECTION_LABELS[section.kind]}</h2></header>
        <div className={`${styles.content} ${section.kind === 'publication' && groups.every(group => group.group.mode === 'tags' || group.entries.every(entry => entry.unit.kind === 'tags')) ? styles.onlyTags : ''}`}>
          {section.kind === 'audio' ? audioRuns(groups).map((run, runIndex, runs) => <div key={run.groups[0]?.group.ref} className={styles.audioRun}>
            <div className={styles.transcriptSummary}><span>{runs.length > 1 ? `口播 ${runIndex + 1} · ` : ''}{transcriptStats(run.groups)}</span>{run.groups.some(group => group.group.mode === 'transcript_paragraph') ? <button type="button" className={styles.focusTrigger} onClick={() => openFocus(run)}><DossierIcon name="expand"/>专注阅读</button> : null}</div>
            <TranscriptGroups groups={run.groups} prefix={prefix}/>
          </div>) : groups.map((value, groupIndex) => <Fragment key={value.group.ref}>
            {section.gaps.filter(gap => gap.beforeGroupRef === value.group.ref).map((gap, gapIndex) => <ImageGap key={gapIndex} pages={gap.missingPages}/>)}
            <DossierGroup value={value} prefix={prefix} previousRole={endingGroupAttribution(groups[groupIndex - 1])}/>
          </Fragment>)}
          {section.gaps.filter(gap => gap.beforeGroupRef === null).map((gap, gapIndex) => <ImageGap key={gapIndex} pages={gap.missingPages}/>)}
        </div>
      </section>)}
      {hasMissing ? <section className={styles.section} id={readingAnchor(prefix, 'reading:missing')} data-section-ref="reading:missing" data-reading-anchor="reading:missing" tabIndex={-1} aria-labelledby={readingAnchor(prefix, 'reading:missing:title')}>
        <header className={styles.sectionIndex}><span className={styles.sectionNumber}>{String(sections.length + 1).padStart(2, '0')}</span><h2 id={readingAnchor(prefix, 'reading:missing:title')}>缺失内容</h2></header>
        <div className={styles.content}>{gaps.map(gap => <div className={styles.missingRow} key={gap.key}><span className={styles.missingTitle}>{gap.label}</span><span className={styles.missingMessage}>{gap.message}</span><span className={styles.missingState}>{gap.state}</span></div>)}<ReadingIssues issues={issues}/></div>
      </section> : null}
      {!sections.length && !hasMissing ? <p className={styles.empty}>本次范围没有可读文字。</p> : null}
      <p className={styles.end}>已显示当前底稿全部可读内容{dossier.selection.mode === 'explicit' ? ' · 当前为选定范围' : ''}</p>
    </article>
    {active && exportSnapshot ? <DossierExportDialog payload={exportSnapshot} domain={domain} onClose={closeModal} onUnavailable={onUnavailable}/> : null}
    {active && focus ? <DossierFocusDialog title={focus.title} groups={focus.groups} memoryKey={`${memoryKey}:focus:${focus.ref}`} onClose={closeModal}/> : null}
  </div>;
}
