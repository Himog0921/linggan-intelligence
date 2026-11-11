import { useEffect, useState } from 'react';
import { c } from './styles';
import { requestJson } from '../../shared/api';
import { bool, decode, num, str } from '../../shared/decode';

type Setting = { domainRef: string; enabled: boolean; revision: number; configurationReady?: boolean };
function setting(value: unknown): Setting {
  return decode(value, row => ({ domainRef: str(row, 'domainRef'), enabled: bool(row, 'enabled'), revision: num(row, 'revision'), configurationReady: row.configurationReady === undefined ? undefined : bool(row, 'configurationReady') }));
}
type Props = { domain: string; onEnabledChange?: (enabled: boolean) => void };
export function TopicAutomation(props: Props) {
  return <DomainAutomation key={props.domain} {...props} />;
}
function DomainAutomation({ domain, onEnabledChange }: Props) {
  const [current, setCurrent] = useState<Setting | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  useEffect(() => { if (current) onEnabledChange?.(current.enabled); }, [current, onEnabledChange]);
  const path = `/api/local/topic-research/automation?${new URLSearchParams({ domainRef: domain })}`;
  useEffect(() => {
    const controller = new AbortController();
    void requestJson(path, { signal: controller.signal }).then(value => {
      if (!controller.signal.aborted) setCurrent(setting(value));
    }).catch(() => { if (!controller.signal.aborted) setError('自动归并设置读取失败'); });
    return () => controller.abort();
  }, [path]);
  async function retry() {
    if (busy) return;
    setBusy(true);
    try { setCurrent(setting(await requestJson(path))); setError(''); }
    catch { setError('自动归并设置读取失败'); }
    finally { setBusy(false); }
  }
  async function toggle() {
    if (!current || busy) return;
    setBusy(true); setError('');
    try {
      setCurrent({ ...setting(await requestJson('/api/local/topic-research/automation', { method: 'POST', body: {
        commandRef: crypto.randomUUID(), domainRef: domain, expectedRevision: current.revision, enabled: !current.enabled,
      } })), configurationReady: current.configurationReady });
    } catch {
      setError('设置未确认');
      try { setCurrent(setting(await requestJson(path))); setError(''); } catch { setCurrent(null); }
    } finally { setBusy(false); }
  }
  return <span>
    {current ? <button className={c('lgi-tm-button')} type="button" role="switch" aria-checked={current.enabled} disabled={busy} onClick={() => void toggle()}>自动归并：{current.enabled ? '开' : '关'}</button>
      : <button className={c('lgi-tm-button')} type="button" disabled aria-busy={!error}>自动归并</button>}
    {current?.enabled && current.configurationReady === false ? <span role="status">待配置</span> : null}
    {error ? <><span role="status">{error}</span>{!current ? <button className={c('lgi-tm-button')} type="button" disabled={busy} onClick={() => void retry()}>重试</button> : null}</> : null}
  </span>;
}
