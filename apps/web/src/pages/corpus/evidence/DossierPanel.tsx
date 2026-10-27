import { ActionButton } from '../../../shared/ActionButton';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useState } from 'react';
import { hideCachedReading } from './dossierPresent';
import { fetchDossier } from './dossierApi';
import { dossierKey } from './evidenceKeys';
import { DossierDocument } from './DossierDocument';
import styles from './DossierDocument.module.css';
interface Props { work: string; domain: string; active: boolean }

/** The dossier tab reads one canonical snapshot; opening it never starts material processing. */
export function DossierPanel({ work, domain, active }: Props) {
  const [activated, setActivated] = useState(active); if (active && !activated) setActivated(true);
  const [revoked, setRevoked] = useState(false);
  const query = useQuery({ queryKey: dossierKey(domain, work), queryFn: ({ signal }) => fetchDossier(work, domain, signal), enabled: activated, staleTime: 30_000, gcTime: 0 });
  const { refetch } = query;
  const refresh = useCallback(() => { void refetch().then(result => { if (result.isSuccess) setRevoked(false); }); }, [refetch]);
  const unavailable = useCallback(() => { setRevoked(true); refresh(); }, [refresh]);
  const payload = revoked || hideCachedReading(query.error) ? null : query.data ?? null;
  if (payload === null) return query.isError || revoked ? <div className={`${styles.surface} ${styles.root}`}><div className={styles.empty}><p role="status">研究底稿暂时不可读。</p><ActionButton className={styles.control} onClick={refresh}>重试读取底稿</ActionButton></div></div> : activated ? <div className={`${styles.surface} ${styles.root}`}><p className={styles.empty} aria-busy="true">正在读取研究底稿…</p></div> : null;
  return <DossierDocument key={`${domain}:${work}`} payload={payload} domain={domain} active={active} busy={query.isFetching} refreshFailed={query.isError} onRefresh={refresh} onUnavailable={unavailable}/>;
}
