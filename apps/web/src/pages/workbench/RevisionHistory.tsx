import { useRef, useState } from 'react';
import { ActionButton } from '../../shared/ActionButton';
import { humanMoment } from '../../shared/formatting';
import * as api from './workbenchApi';
import type { Revision, RevisionSummary } from './workbenchModel';
import styles from './WorkbenchPage.module.css';

export function RevisionHistory({ items, workId, currentRevisionId, disabled, onRestore }: {
  items: RevisionSummary[]; workId: string; currentRevisionId: string;
  disabled: boolean; onRestore: (id: string) => void;
}) {
  return items.map(item => <HistoryEntry key={item.id} item={item} workId={workId}
    disabled={disabled || item.id === currentRevisionId} onRestore={onRestore} />);
}

function HistoryEntry({ item, workId, disabled, onRestore }: {
  item: RevisionSummary; workId: string; disabled: boolean; onRestore: (id: string) => void;
}) {
  const [detail, setDetail] = useState<Revision | null>(null);
  const [failed, setFailed] = useState(false);
  const generation = useRef(0);
  const load = async () => {
    const attempt = ++generation.current;
    setDetail(null); setFailed(false);
    try {
      const revision = await api.readRevision(workId, item.id);
      if (revision.number !== item.number) throw new Error('Revision mismatch');
      if (attempt === generation.current) setDetail(revision);
    } catch {
      if (attempt === generation.current) setFailed(true);
    }
  };
  const readable = item.referenceFormat === 'current';
  const label = readable ? '已保存' : item.referenceFormat === 'legacy_reference' ? '旧版引用' : '格式待核对';
  return <section>
    <div className={styles.row}>
      <strong>版本 {item.number} · {label}</strong><span>{humanMoment(item.savedAt)}</span>
      {readable ? <ActionButton variant="quiet" disabled={disabled} onClick={() => onRestore(item.id)}>恢复为新版本</ActionButton> : null}
    </div>
    {readable ? <details onToggle={event => {
      if (event.currentTarget.open) void load();
      else { generation.current++; setDetail(null); setFailed(false); }
    }}>
      <summary>查看正文</summary>
      {detail ? <pre>{detail.document.paragraphs.map(p => p.text).join('\n\n')}</pre>
        : failed ? <p role="alert">此版本暂时无法安全读取。<ActionButton variant="quiet" onClick={() => void load()}>重新核验</ActionButton></p>
          : <p>正在核验引用</p>}
    </details> : <p>{item.referenceFormat === 'legacy_reference' ? '旧版引用，需重新核验后恢复' : '此版本格式需核对，暂不可恢复'}</p>}
  </section>;
}
