import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useLocation, useNavigationType, useSearchParams } from 'react-router';
import { parseQuery, toPageSearch, type EvidenceQuery } from './evidenceQuery';

/**
 * 地址 ⇄ 查询状态。地址是唯一依据：控件、选中的作品、检查器标签与宽度都由它推出，所以后退／前进
 * 整体还原，不需要 popstate 监听。
 *
 * 所有改动都经过 `go()`：连续两次同步改动叠加而不是互相覆盖（它读的是「最新」而不是上一次渲染的快照）；
 * 改动后的规范地址与当前相同时不写历史，返回 false，由调用方决定是否要重新读取。
 */
export function useEvidenceAddress() {
  const location = useLocation();
  const [params, setParams] = useSearchParams();
  const domainParam = params.get('domain');
  const query = useMemo(() => parseQuery(params), [params]);

  const latest = useRef({ query, domainParam, setParams });
  useEffect(() => {
    latest.current = { query, domainParam, setParams };
  });

  const go = useCallback((update: (current: EvidenceQuery) => EvidenceQuery, replace = false): boolean => {
    const current = latest.current;
    const next = update(current.query);
    const before = toPageSearch(current.domainParam, current.query).toString();
    const after = toPageSearch(current.domainParam, next);
    if (after.toString() === before) return false;
    latest.current = { ...current, query: next };
    current.setParams(after, { replace });
    return true;
  }, []);

  return { location, domainParam, query, go };
}

/**
 * 抽屉（≤1180px）里展开的是哪一件作品；`null` 为收起。
 *
 * 只有两类动作展开它：用户点选一行；从地址还原一个作品（首次进入、直达链接选好领域之后、后退前进）。
 * 列表加载后自动选中第一项不展开——否则手机上一打开页面就被抽屉盖住列表。
 */
export function useDrawerWork(domain: string | null, workInAddress: string | null) {
  const location = useLocation();
  const navigationType = useNavigationType();
  const [work, setWork] = useState<string | null>(null);
  const [seenLocation, setSeenLocation] = useState<string | null>(null);
  const [seenDomain, setSeenDomain] = useState<string | null>(null);
  if (seenLocation !== location.key) {
    setSeenLocation(location.key);
    if (navigationType === 'POP' && workInAddress !== null) setWork(workInAddress);
  }
  // 地址直达一个作品、领域是读到外壳之后才确定的：领域一确定就还原它（旧页由服务器一次渲染好，没有这个间隙）。
  if (seenDomain !== domain) {
    setSeenDomain(domain);
    if (domain !== null && workInAddress !== null) setWork(workInAddress);
  }
  return [work, setWork] as const;
}
