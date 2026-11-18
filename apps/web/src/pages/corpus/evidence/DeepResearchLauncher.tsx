import { useRef, useState } from 'react';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError, isRecord } from '../../../shared/api';
import { InlineNotice } from '../../../shared/InlineNotice';
import { RequestIdentities } from '../../../shared/requestIdentity';
import { SelectField } from '../../../shared/SelectField';
import { DEEP_MODE_LABELS, REASON_COPY, loadSetup, submitDeepRequest, type Mode, type Setup } from './deepResearchApi';
import docStyles from './DossierDocument.module.css';
import styles from './DeepResearchLauncher.module.css';

const MATERIAL_LABELS: Record<string, string> = {
  core_unit: '整篇理解要点', core_evidence: '理解依据', title: '标题', body: '正文', tag: '标签', asr: '口播', cover: '封面与图片',
};
const MAX_QUESTIONS = 4;
const MAX_QUESTION_LENGTH = 120;

function failureCopy(error: unknown): string {
  if (error instanceof ApiError && error.failure.kind === 'http') {
    const body = error.responseBody;
    const reasons = isRecord(body) && Array.isArray(body.reasons) ? body.reasons.filter((r): r is string => typeof r === 'string') : [];
    if (reasons.length) return reasons.map(reason => REASON_COPY[reason] ?? '当前条件不允许发起。').join('');
    if (error.failure.status === 409) return '这篇的理解结果或设置已变化，请刷新后再试。';
    return '发起失败，请稍后再试。';
  }
  if (error instanceof ApiError && error.failure.kind === 'network') return '没有得到服务的答复，这次发起是否生效尚未确认；再次点击会沿用同一次请求。';
  return '发起失败，请稍后再试。';
}

export function DeepResearchLauncher({ domain, work, coreRef, onSubmitted }: {
  domain: string; work: string; coreRef: string; onSubmitted: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [setup, setSetup] = useState<Setup | null>(null);
  const [loadError, setLoadError] = useState(false);
  const [mode, setMode] = useState('');
  const [questions, setQuestions] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const identities = useRef(new RequestIdentities());

  const choose = (modes: Mode[], next: string) => {
    setMode(next);
    setQuestions(modes.find(item => item.researchMode === next)?.defaultSubQuestions ?? []);
  };
  const load = async () => {
    setLoadError(false);
    try {
      const loaded = await loadSetup(domain, work);
      setSetup(loaded);
      choose(loaded.modes, loaded.modes.find(item => item.researchMode === 'content_design')?.researchMode ?? loaded.modes[0]?.researchMode ?? '');
    } catch {
      setLoadError(true);
    }
  };
  const toggle = () => {
    if (open) { setOpen(false); return; }
    setOpen(true);
    setMessage(null);
    if (!setup) void load();
  };
  const current = setup?.modes.find(item => item.researchMode === mode);
  const trimmed = questions.map(text => text.trim());
  const valid = trimmed.length >= 1 && trimmed.length <= MAX_QUESTIONS && trimmed.every(text => text !== '' && [...text].length <= MAX_QUESTION_LENGTH);
  const submit = async () => {
    if (!setup || !current || !valid || busy) return;
    const content = {
      domainRef: domain, workRef: work, coreRef, researchMode: mode, baseMode: null,
      subQuestions: trimmed.map((text, index) => ({ id: `Q${index + 1}`, text })),
      taskConfigRef: setup.taskConfigRef, taskRevision: setup.taskRevision,
    };
    const commandRef = identities.current.keyFor('deep-generate', JSON.stringify(content));
    setBusy(true);
    setMessage(null);
    try {
      await submitDeepRequest({ commandRef, ...content });
      identities.current.settle('deep-generate');
      setOpen(false);
      onSubmitted();
    } catch (error) {
      if (error instanceof ApiError && error.answered) identities.current.settle('deep-generate');
      setMessage(failureCopy(error));
    } finally {
      setBusy(false);
    }
  };

  const quiet = `${docStyles.control} ${docStyles.quiet}`;
  return (
    <>
      <ActionButton variant="primary" className={styles.trigger} aria-expanded={open} onClick={toggle}>
        深入研究
      </ActionButton>
      {open ? (
          <ModalDialog labelledBy="deep-research-title" className={styles.modal} closeOnBackdrop onClose={() => setOpen(false)}>
            <header className={styles.modalHeader}>
              <h2 id="deep-research-title">深入研究</h2>
              <button type="button" data-initial-focus className={styles.close} aria-label="关闭深入研究" onClick={() => setOpen(false)}>×</button>
            </header>
            <div className={styles.modalBody}>
              {loadError ? <InlineNotice tone="error" action={<ActionButton className={quiet} onClick={() => void load()}>重试</ActionButton>}>暂时无法读取深入研究的设置。</InlineNotice> : null}
              {!setup && !loadError ? <p role="status">正在读取设置…</p> : null}
              {setup ? <>
                {setup.unavailable ? <InlineNotice tone="warning">{setup.unavailable}</InlineNotice> : null}
                <SelectField label="研究模式" value={mode} onChange={next => choose(setup.modes, next)}
                  options={setup.modes.map(item => ({ value: item.researchMode, label: DEEP_MODE_LABELS[item.researchMode] ?? item.researchMode }))}/>
                {current ? <>
                  <p className={docStyles.paragraph}>
                    将使用的材料：{current.defaultMaterialKinds.map(kind => MATERIAL_LABELS[kind] ?? kind).join('、')}
                    {current.defaultMaterialKinds.includes('cover') ? `（图片 ${Math.min(setup.imageCount, setup.imageLimit)} 张，最多 ${setup.imageLimit} 张）` : ''}
                  </p>
                  <fieldset className={styles.group}>
                    <legend className={styles.label}>子问题（最多 {MAX_QUESTIONS} 个，每个不超过 {MAX_QUESTION_LENGTH} 字）</legend>
                    {questions.map((text, index) => <div className={styles.row} key={index}>
                      <input className={styles.question} aria-label={`子问题 ${index + 1}`} value={text} maxLength={MAX_QUESTION_LENGTH}
                        onChange={event => setQuestions(questions.map((item, at) => at === index ? event.target.value : item))}/>
                      <ActionButton className={quiet} disabled={questions.length <= 1}
                        onClick={() => setQuestions(questions.filter((_, at) => at !== index))}>删除</ActionButton>
                    </div>)}
                    <div className={styles.row}>
                      <ActionButton className={quiet} disabled={questions.length >= MAX_QUESTIONS}
                        onClick={() => setQuestions([...questions, ''])}>添加子问题</ActionButton>
                    </div>
                  </fieldset>
                  <div className={styles.row}>
                    <ActionButton variant="primary" busy={busy} disabled={!valid || setup.unavailable !== null} onClick={() => void submit()}>提交</ActionButton>
                  </div>
                </> : null}
              </> : null}
              {message ? <InlineNotice>{message}</InlineNotice> : null}
            </div>
          </ModalDialog>
      ) : null}
    </>
  );
}
