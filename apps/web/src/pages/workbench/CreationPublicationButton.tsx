import { useRef, useState } from 'react';
import { Link } from 'react-router';
import { ActionButton } from '../../shared/ActionButton';
import { NativePublicationDialog } from '../ops/NativePublicationDialog';
import { creationPublicationHref, type Account } from '../ops/api';

type SavedDraft = {
  revisionId: string;
  title: string;
  account: Pick<Account, 'ref' | 'name' | 'platform'> | null;
};

export function CreationPublicationButton({ workId, domainRef, disabled, saveCurrent, onRegistered }: {
  workId: string;
  domainRef: string;
  disabled?: boolean;
  saveCurrent: () => Promise<SavedDraft>;
  onRegistered?: () => void;
}) {
  const [draft, setDraft] = useState<SavedDraft | null>(null);
  const [publicationRef, setPublicationRef] = useState<string | null>(null);
  const [message, setMessage] = useState('');
  const [saving, setSaving] = useState(false);
  const savingDraft = useRef(false);

  async function open() {
    if (savingDraft.current) return;
    savingDraft.current = true;
    setSaving(true);
    setMessage('');
    try {
      const saved = await saveCurrent();
      if (!saved.account) {
        setMessage('请先为稿件选择发布账号。');
        return;
      }
      setDraft(saved);
    } catch {
      setMessage('稿件保存未完成。请重试。');
    } finally {
      savingDraft.current = false;
      setSaving(false);
    }
  }

  return <>
    <ActionButton variant="quiet" disabled={disabled || saving} onClick={() => void open()}>
      {saving ? '正在保存稿件…' : '登记发布'}
    </ActionButton>
    {message ? <span role="status">{message}</span> : null}
    {publicationRef ? <Link to={creationPublicationHref(domainRef, publicationRef)}>查看发布记录</Link> : null}
    {draft?.account ? <NativePublicationDialog
      accounts={[draft.account]}
      close={() => setDraft(null)}
      draft={{ workId, expectedRevision: draft.revisionId, title: draft.title, onRegistered: ref => {
        setPublicationRef(ref);
        setMessage('发布已登记。');
        onRegistered?.();
      } }}
    /> : null}
  </>;
}
