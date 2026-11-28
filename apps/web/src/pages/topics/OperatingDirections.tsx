import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { requestJson } from '../../shared/api';
import {
  bool,
  decode,
  list,
  num,
  obj,
  optNum,
  optObj,
  optStr,
  str,
  strList,
} from '../../shared/decode';
import { DirectionGeneration, type Candidate } from './DirectionGeneration';
import { creationPublicationHref } from '../ops/api';
import styles from './OperatingDirections.module.css';

interface Direction {
  directionRef: string;
  label: string;
  definition: string;
  topicRefs: string[];
  textSource: string[] | null;
  textSources?: string[][];
  evidenceWorkRefs?: string[];
  candidateSource?: string[] | null;
}
interface Catalog {
  directions: Direction[];
  benchmarkAuthors: string[];
}
function parse(value: unknown) {
  return decode(value, (root) => {
    const draft = obj(root, 'draft'),
      published = optObj(root, 'published');
    return {
      revision: num(root, 'revision'),
      publishedRevision: optNum(root, 'publishedRevision'),
      total: num(root, 'totalWorks'),
      topics: list(root, 'topics', (t) => ({
        id: str(t, 'id'),
        label: str(t, 'label'),
        definition: str(t, 'definition'),
        version: str(t, 'definitionRef'),
        samples: list(t, 'samples', (w) => ({
          id: str(w, 'id'),
          title: optStr(w, 'title'),
        })),
        parent: optStr(t, 'parent'),
      })),
      authors: list(root, 'authors', (a) => ({
        id: str(a, 'id'),
        name: str(a, 'name'),
      })),
      draft: {
        directions: list(draft, 'directions', (d) => ({
          directionRef: str(d, 'directionRef'),
          label: str(d, 'label'),
          definition: str(d, 'definition'),
          topicRefs: strList(d, 'topicRefs'),
          candidateSource:
            d.candidateSource == null ? null : strList(d, 'candidateSource'),
          textSources: Array.isArray(d.textSources)
            ? d.textSources.map((pair: unknown) => strList({ pair }, 'pair'))
            : [],
          evidenceWorkRefs: strList(d, 'evidenceWorkRefs'),
          textSource: d.textSource == null ? null : strList(d, 'textSource'),
        })),
        benchmarkAuthors: strList(draft, 'benchmarkAuthors'),
      },
      published: published && {
        assigned: num(published, 'assignedWorks'),
        directions: list(published, 'directions', (d) => ({
          id: str(d, 'id'),
          label: str(d, 'label'),
          definition: str(d, 'definition'),
          ownTotal: num(d, 'ownTotal'),
          ownKnown: num(d, 'ownKnown'),
          ownHits: num(d, 'ownHits'),
          externalTotal: num(d, 'externalTotal'),
          externalKnown: num(d, 'externalKnown'),
          externalHits: num(d, 'externalHits'),
          works: list(d, 'works', (w) => ({
            id: str(w, 'workRef'),
            title: optStr(w, 'title'),
            ownHit: bool(w, 'ownHit'),
            benchmark: bool(w, 'benchmark'),
            likes: optNum(w, 'likes'),
            publications: list(w, 'publications', (p) => ({
              id: str(p, 'publicationRef'),
              name: optStr(p, 'accountName'),
              views: optNum(p, 'views'),
            })),
          })),
        })),
      },
    };
  });
}
export function OperatingDirections({ domain, triggerContainer }: { domain: string; triggerContainer?: HTMLElement | null }) {
  const [editMappings, setEditMappings] = useState(false);
  const [editAuthors, setEditAuthors] = useState(false);
  const [authorQuery, setAuthorQuery] = useState('');
  const [open, setOpen] = useState(false),
    [epoch, reload] = useState(0);
  const [data, setData] = useState<ReturnType<typeof parse>>(),
    [draft, setDraft] = useState<Catalog>();
  const [message, setMessage] = useState(''),
    [busy, setBusy] = useState(false);
  const pending = useRef<{ key: string; id: string } | null>(null);
  useEffect(() => {
    if (!open) return;
    const controller = new AbortController();

    void requestJson(
      '/api/local/topic-map/directions?' +
        new URLSearchParams({ domainRef: domain }),
      { signal: controller.signal },
    )
      .then(parse)
      .then((v) => {
        if (!controller.signal.aborted) {
          setData(v);
          setDraft(v.draft);
        }
      })
      .catch(() => {
        if (!controller.signal.aborted) setMessage('读取失败，请重新读取。');
      });
    return () => controller.abort();
  }, [domain, open, epoch]);
  const patch = (id: string, fields: Partial<Direction>) =>
    setDraft(
      (previous) =>
        previous && {
          ...previous,
          directions: previous.directions.map((d) =>
            d.directionRef === id ? { ...d, ...fields } : d,
          ),
        },
    );
  const [preview, setPreview] = useState<{
    signature: string;
    hash: string;
    before: number;
    after: number;
    changed: number;
    from: number;
    to: number;
  }>();
  const signature = JSON.stringify([data?.revision, draft]);
  async function showImpact() {
    if (!data || !draft) return;
    setBusy(true);
    try {
      const result = await requestJson(
        '/api/local/topic-map/directions/preview',
        {
          method: 'POST',
          body: {
            domainRef: domain,
            expectedRevision: data.revision,
            publish: true,
            catalog: draft,
            commandRef: crypto.randomUUID(),
          },
        },
      );
      setPreview(
        decode(result, (r) => ({
          signature,
          hash: str(r, 'previewHash'),
          before: num(r, 'before'),
          after: num(r, 'after'),
          changed: num(r, 'changedWorks'),
          from: num(r, 'priorDirections'),
          to: num(r, 'nextDirections'),
        })),
      );
    } catch {
      setMessage('预览未完成，请核对方向名、定义及每个方向的来源主题。');
    } finally {
      setBusy(false);
    }
  }
  async function save(publish: boolean) {
    if (!data || !draft || busy) return;
    const body = {
      domainRef: domain,
      expectedRevision: data.revision,
      publish,
      catalog: draft,
      previewHash: publish ? preview?.hash : null,
    };
    const key = JSON.stringify(body);
    if (pending.current?.key !== key)
      pending.current = { key, id: crypto.randomUUID() };
    setBusy(true);
    try {
      await requestJson('/api/local/topic-map/directions', {
        method: 'POST',
        body: { ...body, commandRef: pending.current.id },
      });
      pending.current = null;
      setMessage(publish ? '方向清单已发布。' : '草案已保存。');
      reload((v) => v + 1);
    } catch {
      setMessage(
        '保存未确认。可重试原操作；如果已有新版本，请重新读取后核对修改。',
      );
    } finally {
      setBusy(false);
    }
  }
  const direct = new Set(draft?.directions.flatMap((d) => d.topicRefs));
  const proposed =
    data?.topics.filter(
      (t) =>
        !direct.has(t.id) &&
        (!t.parent || !data.topics.some((p) => p.id === t.parent)),
    ) ?? [];
  function adopt(candidates: Candidate[]) {
    if (!data || !draft) return;
    let directions = draft.directions;
    for (const candidate of candidates) {
      const id = candidate.existing ?? candidate.id;
      const existing = directions.find((d) => d.directionRef === id);
      if (candidate.existing && !existing) {
        setMessage('对应方向已退出草案，请重新核对。');
        return;
      }
      const sources = candidate.topics.map((topic, i) => {
        const current = data.topics.find(
          (t) => t.id === topic && t.version === candidate.versions[i],
        );
        return current ? [current.id, current.version] : [];
      });
      if (!sources.length || sources.some((pair) => !pair.length)) {
        setMessage('候选依据已变化，请刷新方向和生成记录。');
        return;
      }
      const merged: Direction = existing
        ? {
            ...existing,
            topicRefs: [
              ...new Set([...existing.topicRefs, ...candidate.topics]),
            ],
          }
        : {
            directionRef: id,
            label: candidate.label,
            definition: candidate.definition,
            topicRefs: candidate.topics,
            candidateSource: [candidate.run, candidate.id],
            textSource: null,
            textSources: sources,
            evidenceWorkRefs: candidate.works,
          };
      directions = directions
        .filter((d) => d.directionRef !== id)
        .flatMap((d) => {
          const topicRefs = d.topicRefs.filter(
            (t) => !candidate.topics.includes(t),
          );
          return topicRefs.length || !d.topicRefs.length
            ? [{ ...d, topicRefs }]
            : [];
        });
      directions = [...directions, merged];
    }
    setDraft({ ...draft, directions });
    setMessage('已将候选主题归入草案，可预览受影响作品。');
  }
  if (!open) {
    const trigger = (
      <button type="button" className={triggerContainer ? styles.contextTrigger : undefined} onClick={() => setOpen(true)}>
        查看运营方向
      </button>
    );
    return triggerContainer ? createPortal(trigger, triggerContainer) : trigger;
  }
  return (
    <section aria-label="运营方向" className={styles.panel}>
      <h2>运营方向</h2>
      <button
        type="button"
        onClick={() => {
          setData(undefined);
          reload((v) => v + 1);
        }}
      >
        重新读取
      </button>
      {message && <p role="status">{message}</p>}
      {!data || !draft ? (
        <p>正在读取方向清单</p>
      ) : (
        <>
          {data.published ? (
            <>
              <p>
                已发布第 {data.publishedRevision} 版 · 已归类{' '}
                {data.published.assigned} / {data.total} 篇，待归类{' '}
                {data.total - data.published.assigned} 篇
              </p>
              {data.published.directions.map((d) => (
                <details key={d.id}>
                  <summary>
                    {d.label} · {d.works.length} 篇 · 我方阅读达标{' '}
                    {d.ownKnown
                      ? ((100 * d.ownHits) / d.ownKnown).toFixed(0) + '%'
                      : '暂无已知阅读'}{' '}
                    · 对标高赞 {d.externalHits} 篇
                  </summary>
                  <p>{d.definition}</p>
                  <p>
                    我方小红书阅读 ≥5,000：{d.ownHits} 篇，已知 {d.ownKnown} /
                    已发布 {d.ownTotal} 篇，命中率{' '}
                    {d.ownKnown
                      ? ((100 * d.ownHits) / d.ownKnown).toFixed(1) + '%'
                      : '暂无可计量发布'}
                    。
                  </p>
                  <p>
                    家长向对标点赞 ≥200：{d.externalHits} 篇，点赞已知{' '}
                    {d.externalKnown} / 对标 {d.externalTotal} 篇。
                  </p>
                  <ul>
                    {d.works.map((w) => (
                      <li key={w.id}>
                        <a
                          href={
                            '/corpus/evidence?' +
                            new URLSearchParams({ domain, work: w.id })
                          }
                        >
                          {w.title || '查看作品依据'}
                        </a>
                        {w.publications.map((p) => (
                          <a
                            key={p.id}
                            href={creationPublicationHref(domain, p.id)}
                          >
                            {' '}
                            · {p.name || '我方发布'} 阅读 {p.views ?? '未知'}
                          </a>
                        ))}
                        {w.ownHit ? ' · 我方阅读达标' : ''}
                        {w.benchmark
                          ? ' · 对标点赞 ' + (w.likes ?? '未知')
                          : ''}
                      </li>
                    ))}
                  </ul>
                </details>
              ))}
            </>
          ) : (
            <p>方向清单待确认，当前领域共有 {data.total} 篇作品。</p>
          )}
          <details open={!data.published || undefined}>
            <summary>编辑方向草案（{draft.directions.length} 个方向）</summary>
            <p>
              首次打开时列出覆盖作品最多的根主题，供手工起草。AI
              可以跨主题归纳方向。发布后，新归入这些主题的作品会随方向更新。
            </p>
            <DirectionGeneration
              domain={domain}
              unmapped={proposed.map((t) => t.id)}
              onAdopt={adopt}
            />
            <fieldset disabled={busy}>
              {draft.directions.map((d) => (
                <details key={d.directionRef}>
                  <summary>{d.label || '新方向'}</summary>
                  <label>
                    方向名{' '}
                    <input
                      value={d.label}
                      onChange={(e) =>
                        patch(d.directionRef, { label: e.target.value })
                      }
                    />
                  </label>
                  <label>
                    定义{' '}
                    <textarea
                      value={d.definition}
                      onChange={(e) =>
                        patch(d.directionRef, { definition: e.target.value })
                      }
                    />
                  </label>
                  <ul>
                    {d.topicRefs.map((id) => {
                      const t = data.topics.find((t) => t.id === id);
                      return (
                        <li key={id}>
                          {t?.label ?? '来源主题已退出当前范围'}{' '}
                          {t?.samples.map((w) => (
                            <a
                              key={w.id}
                              href={
                                '/corpus/evidence?' +
                                new URLSearchParams({ domain, work: w.id })
                              }
                            >
                              {w.title || '作品依据'}{' '}
                            </a>
                          ))}
                          {!t && (
                            <button
                              type="button"
                              onClick={() =>
                                patch(d.directionRef, {
                                  topicRefs: d.topicRefs.filter(
                                    (x) => x !== id,
                                  ),
                                })
                              }
                            >
                              移除失效映射
                            </button>
                          )}
                        </li>
                      );
                    })}
                  </ul>
                  {(d.label === '来源已变化的方向' ||
                    (d.textSource &&
                      !data.topics.some(
                        (t) =>
                          t.id === d.textSource?.[0] &&
                          t.version === d.textSource?.[1],
                      ))) && (
                    <button
                      type="button"
                      onClick={() =>
                        patch(d.directionRef, {
                          textSource: null,
                          textSources: [],
                          evidenceWorkRefs: [],
                          candidateSource: null,
                          label: '',
                          definition: '',
                        })
                      }
                    >
                      重新填写方向说明
                    </button>
                  )}
                  <button
                    type="button"
                    onClick={() =>
                      setDraft({
                        ...draft,
                        directions: draft.directions.filter(
                          (i) => i.directionRef !== d.directionRef,
                        ),
                      })
                    }
                  >
                    移出草案
                  </button>
                </details>
              ))}
              <button
                type="button"
                onClick={() =>
                  setDraft({
                    ...draft,
                    directions: [
                      ...draft.directions,
                      {
                        directionRef: crypto.randomUUID(),
                        label: '',
                        definition: '',
                        topicRefs: [],
                        textSource: null,
                      },
                    ],
                  })
                }
              >
                新增方向
              </button>
              {proposed.length > 0 && (
                <details>
                  <summary>待确认的新方向候选（{proposed.length}）</summary>
                  {proposed.map((t) => (
                    <div key={t.id}>
                      <strong>{t.label}</strong>
                      <p>{t.definition}</p>
                      <button
                        type="button"
                        onClick={() =>
                          setDraft({
                            ...draft,
                            directions: [
                              ...draft.directions,
                              {
                                directionRef: crypto.randomUUID(),
                                label: t.label,
                                definition: t.definition,
                                topicRefs: [t.id],
                                textSource: [t.id, t.version],
                              },
                            ],
                          })
                        }
                      >
                        加入方向草案
                      </button>
                    </div>
                  ))}
                </details>
              )}
              <details onToggle={(e) => setEditMappings(e.currentTarget.open)}>
                <summary>分配研究主题（{data.topics.length}）</summary>
                {editMappings &&
                  data.topics.map((t) => (
                    <label key={t.id} title={t.definition}>
                      {t.label}
                      <select
                        aria-label={t.label + '的运营方向'}
                        value={
                          draft.directions.find((d) =>
                            d.topicRefs.includes(t.id),
                          )?.directionRef ?? ''
                        }
                        onChange={(e) =>
                          setDraft({
                            ...draft,
                            directions: draft.directions.map((d) => ({
                              ...d,
                              topicRefs: [
                                ...d.topicRefs.filter((id) => id !== t.id),
                                ...(d.directionRef === e.target.value
                                  ? [t.id]
                                  : []),
                              ],
                            })),
                          })
                        }
                      >
                        <option value="">随上级主题，未匹配时待定</option>
                        {draft.directions.map((d) => (
                          <option key={d.directionRef} value={d.directionRef}>
                            {d.label || '新方向'}
                          </option>
                        ))}
                      </select>
                    </label>
                  ))}
              </details>
              <details onToggle={(e) => setEditAuthors(e.currentTarget.open)}>
                <summary>
                  家长向对标作者（{draft.benchmarkAuthors.length}）
                </summary>
                {draft.benchmarkAuthors
                  .filter((id) => !data.authors.some((a) => a.id === id))
                  .map((id) => (
                    <button
                      type="button"
                      key={id}
                      onClick={() =>
                        setDraft({
                          ...draft,
                          benchmarkAuthors: draft.benchmarkAuthors.filter(
                            (a) => a !== id,
                          ),
                        })
                      }
                    >
                      移除已退出范围的作者
                    </button>
                  ))}
                {editAuthors && (
                  <label>
                    查找对标作者{' '}
                    <input
                      type="search"
                      value={authorQuery}
                      onChange={(e) => setAuthorQuery(e.target.value)}
                    />
                  </label>
                )}
                {editAuthors &&
                  data.authors
                    .filter(
                      (a) =>
                        a.name.includes(authorQuery.trim()) ||
                        a.id.includes(authorQuery.trim()),
                    )
                    .map((a) => (
                      <label key={a.id}>
                        <input
                          type="checkbox"
                          checked={draft.benchmarkAuthors.includes(a.id)}
                          onChange={(e) =>
                            setDraft({
                              ...draft,
                              benchmarkAuthors: e.target.checked
                                ? [...draft.benchmarkAuthors, a.id]
                                : draft.benchmarkAuthors.filter(
                                    (id) => id !== a.id,
                                  ),
                            })
                          }
                        />
                        {a.name}
                      </label>
                    ))}
              </details>
              <button type="button" onClick={() => void save(false)}>
                保存草案
              </button>
              <button type="button" onClick={() => void showImpact()}>
                预览发布影响
              </button>
              {preview?.signature === signature && (
                <>
                  <p>
                    方向 {preview.from} → {preview.to} 个，覆盖 {preview.before}{' '}
                    → {preview.after} 篇，{preview.changed}{' '}
                    篇作品的方向归属将发生变化。
                  </p>
                  <button type="button" onClick={() => void save(true)}>
                    确认并发布方向
                  </button>
                </>
              )}
            </fieldset>
          </details>
        </>
      )}
    </section>
  );
}
