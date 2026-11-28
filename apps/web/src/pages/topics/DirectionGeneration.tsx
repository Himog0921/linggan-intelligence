import { useEffect, useRef, useState } from 'react';
import { ApiError, isRecord, requestJson } from '../../shared/api';
import { decode, list, obj, optStr, str, strList } from '../../shared/decode';

export interface Candidate {
  id: string;
  run: string;
  existing: string | null;
  label: string;
  definition: string;
  topics: string[];
  versions: string[];
  works: string[];
}
const endpoint = '/api/local/topic-map/directions/runs';
function readRuns(value: unknown) {
  return decode(value, (root) =>
    list(root, 'runs', (run) => ({
      id: str(run, 'runRef'),
      state: str(run, 'state'),
      reason: optStr(run, 'reason'),
      candidates: list(run, 'candidates', (c) => ({
        id: str(c, 'directionRef'),
        run: str(run, 'runRef'),
        existing: optStr(c, 'existingDirectionRef'),
        label: str(c, 'label'),
        definition: str(c, 'definition'),
        topics: strList(c, 'topicRefs'),
        versions: strList(c, 'topicRefs').map((t) =>
          str(obj(c, 'topicDefinitionRefs'), t),
        ),
        works: strList(c, 'evidenceWorkRefs'),
      })),
    })),
  );
}
function errorMessage(error: unknown) {
  if (error instanceof ApiError && isRecord(error.responseBody)) {
    const { reasons, error: reason } = error.responseBody;
    if (Array.isArray(reasons)) return '生成条件未满足：' + reasons.join('、');
    if (typeof reason === 'string') return '生成请求未完成：' + reason;
  }
  return '未收到生成回执。可重试同一请求。';
}
export function DirectionGeneration({
  domain,
  unmapped,
  onAdopt,
}: {
  domain: string;
  unmapped: string[];
  onAdopt: (candidates: Candidate[]) => void;
}) {
  const [runs, setRuns] = useState<ReturnType<typeof readRuns>>([]);
  const [message, setMessage] = useState('');
  const [epoch, reload] = useState(0);
  const [busy, setBusy] = useState(false);
  const pending = useRef<{ key: string; id: string } | null>(null);
  useEffect(() => {
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try {
        const result = readRuns(
          await requestJson(
            endpoint + '?' + new URLSearchParams({ domainRef: domain }),
            { signal: controller.signal },
          ),
        );
        if (controller.signal.aborted) return;
        setRuns(result);
        if (
          result.some((r) =>
            ['pending', 'queued', 'running', 'ready'].includes(r.state),
          )
        )
          timer = setTimeout(refresh, 5000);
      } catch {
        if (!controller.signal.aborted) setMessage('生成记录暂时无法读取。');
      }
    }
    void refresh();
    return () => {
      controller.abort();
      clearTimeout(timer);
    };
  }, [domain, epoch]);
  async function generate(proposal: boolean) {
    const body = {
      domainRef: domain,
      taskKey: proposal ? 'direction_proposal' : 'direction_list_draft',
      topicRefs: proposal ? unmapped : [],
    };
    const key = JSON.stringify(body);
    if (pending.current?.key !== key)
      pending.current = { key, id: crypto.randomUUID() };
    setBusy(true);
    try {
      await requestJson(endpoint, {
        method: 'POST',
        body: { ...body, commandRef: pending.current.id },
      });
      pending.current = null;
      setMessage('已提交生成任务。候选需逐项采纳后发布。');
      reload((v) => v + 1);
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setBusy(false);
    }
  }
  return (
    <details>
      <summary>AI 方向初稿与增量建议</summary>
      <button
        type="button"
        disabled={busy}
        onClick={() => void generate(false)}
      >
        根据当前材料归纳方向
      </button>
      <button
        type="button"
        disabled={busy || !unmapped.length}
        onClick={() => void generate(true)}
      >
        为待归类主题生成建议（{unmapped.length}）
      </button>
      <button type="button" onClick={() => reload((v) => v + 1)}>
        刷新生成记录
      </button>
      {message && <p role="status">{message}</p>}
      {runs.map((r) => (
        <section key={r.id}>
          <p>
            生成状态 · {r.state}
            {r.reason && ' · ' + r.reason}
          </p>
          {r.candidates.length > 1 && (
            <button type="button" onClick={() => onAdopt(r.candidates)}>
              将本次全部候选纳入草案
            </button>
          )}
          {r.candidates.map((c) => (
            <article key={c.id}>
              <strong>{c.label}</strong>
              <p>{c.definition}</p>
              {c.works.map((id, i) => (
                <a
                  key={id}
                  href={
                    '/corpus/evidence?' +
                    new URLSearchParams({ domain, work: id })
                  }
                >
                  作品依据 {i + 1}{' '}
                </a>
              ))}
              <button type="button" onClick={() => onAdopt([c])}>
                {c.existing ? '并入已有方向草案' : '采用候选并调整主题归属'}
              </button>
            </article>
          ))}
        </section>
      ))}
    </details>
  );
}
