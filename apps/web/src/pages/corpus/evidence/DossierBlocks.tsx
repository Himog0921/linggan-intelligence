import type { DossierBlock, DossierIssue, Speaker } from './dossierTypes';
import styles from './Dossier.module.css';

const SPEAKERS: Record<Speaker, string> = { author: '作品作者', other: '他人表述', unconfirmed: '表达者未确认' };
export function DossierIssues({ issues }: { issues: readonly DossierIssue[] }) {
  if (issues.length === 0) return null;
  return <ul className={styles.issues}>{[...new Map(issues.map(issue => [issue.key, issue])).values()].map(issue => <li key={issue.key}>{issue.message}</li>)}</ul>;
}
function Content({ block }: { block: DossierBlock }) {
  switch (block.kind) {
    case 'heading': {
      const Heading = `h${Math.min(6, 3 + (block.level ?? 1))}` as 'h4' | 'h5' | 'h6';
      return <Heading className={styles.blockHeading} data-source-level={block.level}>{block.text}</Heading>;
    }
    case 'quote': return <blockquote className={styles.text}>{block.text}</blockquote>;
    case 'gap': return <p className={styles.gap}>{block.text.startsWith('〔') && block.text.endsWith('〕') ? block.text : `〔${block.text}〕`}</p>;
    case 'list': return block.ordered
      ? <ol className={styles.blockList} start={block.start}>{block.items.map((text, index) => <li key={index}>{text}</li>)}</ol>
      : <ul className={styles.blockList}>{block.items.map((text, index) => <li key={index}>{text}</li>)}</ul>;
    case 'table': return <div className={styles.tableScroll} role="region" aria-label="原文表格" tabIndex={0}><table className={styles.blockTable}><thead><tr>{block.columns.map((text, index) => <th scope="col" key={index}>{text}</th>)}</tr></thead><tbody>{block.rows.map((row, index) => <tr key={index}>{row.map((text, column) => <td key={column}>{text}</td>)}</tr>)}</tbody></table></div>;
    case 'dialogue': return <dl className={styles.dialogue}>{block.turns.map((turn, index) => <div key={index}><dt>{SPEAKERS[turn.speaker]}</dt><dd>{turn.text}</dd></div>)}</dl>;
    case 'paragraph': return <p className={styles.text}>{block.text}</p>;
  }
}
/** Continuous full reading. Structure and attribution stay with the source words. */
export function DossierBlocks({ blocks, issues = [] }: { blocks: readonly DossierBlock[]; issues?: readonly DossierIssue[] }) {
  const localKeys = new Set(blocks.flatMap(block => (block.issues ?? []).map(issue => issue.key)));
  return <div className={styles.blocks}>{blocks.map((block, index) => <div key={block.ref} className={styles.block} data-block-ref={block.ref}>
    {block.speaker && block.speaker !== 'author' && block.kind !== 'dialogue' && (index === 0 || blocks[index - 1]?.speaker !== block.speaker || blocks[index - 1]?.kind === 'dialogue') ? <p className={styles.attribution}>{SPEAKERS[block.speaker]}</p> : null}
    {block.time ? <p className={styles.attribution}>{block.time.startMs === block.time.endMs ? `${(block.time.startMs / 1000).toFixed(3)} 秒` : `${(block.time.startMs / 1000).toFixed(3)}–${(block.time.endMs / 1000).toFixed(3)} 秒`}</p> : null}
    <Content block={block} />
    <DossierIssues issues={[...(block.issues ?? []), ...issues.filter(issue => issue.blockRef === block.ref && !localKeys.has(issue.key))]} />
  </div>)}<DossierIssues issues={issues.filter(issue => !issue.blockRef && !localKeys.has(issue.key))} /></div>;
}
