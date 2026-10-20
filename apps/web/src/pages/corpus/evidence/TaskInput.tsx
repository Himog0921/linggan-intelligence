import { useEffect, useRef, useState, type Ref } from 'react';
import { fetchCreatorPreview } from './dossierApi';
import { TASK_OPTIONS, fieldName, taskNote } from './dossierPresent';
import type { CreatorPreview, DossierPayload, TaskKey } from './dossierTypes';
import { Section } from './Primitives';
import styles from './Dossier.module.css';

function Disclosure({ title, text }: { title: string; text: string }) {
  return (
    <details className={styles.disclosure}>
      <summary>{title}</summary>
      <pre className={styles.pre}>{text}</pre>
    </details>
  );
}

type Preview = { phase: 'idle' } | { phase: 'loading' } | { phase: 'error' } | { phase: 'done'; result: CreatorPreview };

function PreviewResult({ preview }: { preview: Preview }) {
  if (preview.phase === 'error') return <p className={styles.note}>完整请求预览暂不可用，请重试。</p>;
  if (preview.phase !== 'done') return null;
  const { result } = preview;
  if (!result.available) {
    return <p className={styles.note}>{result.reason === 'model_not_configured' ? '旧配置尚未指定模型，以上为旧任务默认材料预览。' : '当前无法准备完整请求，请检查模型输入额度或材料范围。'}</p>;
  }
  return (
    <>
      <p>{`模型输入上限 ${result.inputTokenLimit ?? '未知'} tokens；输入保守估算上界 ${result.estimatedInputTokensUpperBound ?? '未知'} tokens。这是旧配置下的预览，不会启用任务。`}</p>
      {result.materials ? <Disclosure title="实际准备的 Markdown 材料" text={result.materials} /> : null}
      {/* 完整请求含系统提示词与整份输入：只作折叠诊断显示，不写存储、不写日志（P08）。 */}
      <Disclosure title="完整请求与来源映射" text={JSON.stringify(result.raw, null, 2)} />
    </>
  );
}

/**
 * 「送给研究任务的材料」：选一个研究任务，看它实际会拿到的材料——已纳入的字符数、是否有预算截断、各片段、标准 Markdown。
 * 创作者任务另有「按旧配置预览完整请求」：只读，不调用模型、不预留额度，只在用户点击后读取。
 */
export function TaskInput({ payload, work, domain, sectionRef }: { payload: DossierPayload; work: string; domain: string; sectionRef: Ref<HTMLDivElement> }) {
  const [task, setTask] = useState<TaskKey>('creator');
  const [preview, setPreview] = useState<Preview>({ phase: 'idle' });
  const controller = useRef<AbortController | null>(null);
  useEffect(() => () => controller.current?.abort(), []);
  const input = payload.inputs[task];

  function readPreview() {
    controller.current?.abort();
    const abort = new AbortController();
    controller.current = abort;
    setPreview({ phase: 'loading' });
    fetchCreatorPreview(work, domain, abort.signal).then(
      result => {
        if (!abort.signal.aborted) setPreview({ phase: 'done', result });
      },
      () => {
        if (!abort.signal.aborted) setPreview({ phase: 'error' });
      },
    );
  }

  return (
    <div ref={sectionRef} className={styles.anchor}>
      <Section title="旧任务输入预览">
        <p className={styles.note}>以下是旧任务材料与旧配置预览，当前执行已停用；不代表新统一设置或每篇解读的实际模型请求。</p>
        <label className={styles.task}>
          选择任务
          <select
            aria-label="选择研究任务"
            value={task}
            onChange={event => {
              setTask(event.currentTarget.value as TaskKey);
              setPreview({ phase: 'idle' });
            }}
          >
            {TASK_OPTIONS.map(([value, label]) => (
              <option key={value} value={value}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <p className={styles.note}>{taskNote(task)}</p>
        <p>{`已纳入 ${input.includedCharacterCount} 字符${input.truncated ? ' · 有材料因预算未纳入' : ' · 本层没有预算截断'}`}</p>
        {input.fragments.length === 0 ? <p>当前没有可用于这项研究的作品文字。</p> : null}
        {input.fragments.map((fragment, index) => (
          <Disclosure key={index} title={`${fieldName(fragment.field)} · ${fragment.characterCount} 字符`} text={fragment.text} />
        ))}
        {input.markdown ? <Disclosure title="查看标准 Markdown 材料" text={input.markdown} /> : null}
        {task === 'creator' ? (
          <>
            <button type="button" disabled={preview.phase === 'loading'} onClick={readPreview}>
              按旧配置预览完整请求
            </button>
            <PreviewResult preview={preview} />
          </>
        ) : null}
        <Disclosure title="查看这份任务材料 JSON" text={JSON.stringify(input.raw, null, 2)} />
      </Section>
    </div>
  );
}
