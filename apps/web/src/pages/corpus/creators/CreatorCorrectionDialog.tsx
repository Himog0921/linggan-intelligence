import { useId, useRef, useState, type FormEvent } from 'react';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { feedbackForError, type Feedback } from './creatorsFeedback';
import { CRITERIA, FIELD_NAMES, fieldSource } from './creatorsPresent';
import type { Fragment } from './creatorsTypes';
import { FeedbackLine } from './FeedbackLine';
import { useOverrideMutation } from './useCreatorWrites';
import { useWriteGuard } from './useWriteGuard';
import styles from './CreatorDialogs.module.css';

/** 要调整的一项判断。作品表达与作者倾向是两类不同的对象，这里只调整被选中的这一项。 */
export interface CorrectionTarget {
  domain: string;
  domainName: string;
  creatorKey: string;
  /** 作者层判断（作者倾向、机构／品牌自述）为 null。 */
  workRef: string | null;
  field: string;
  subjectTitle: string;
  currentValue: string;
  /** 已有的人工调整；有它才能「恢复自动判断」。 */
  manual: { reason: string | null; supportFragmentIds: string[] } | null;
  supports: Fragment[];
  /** 用来给支持片段起人话分组名：作品编号 → 标题。 */
  workTitles: ReadonlyMap<string, string>;
}

interface CorrectionDialogProps {
  target: CorrectionTarget;
  onSaved: () => void;
  onClose: () => void;
}

const CHOICES: Record<string, readonly (readonly [string, string])[]> = {
  relevance: [['related', '与当前领域相关'], ['unrelated', '与当前领域无关'], ['unknown', '尚不能判定']],
  focus: [['vertical_tendency', '本领域持续创作'], ['multi_topic', '多主题分享'], ['unknown', '尚不能判定']],
};
const TRAIT_CHOICES: readonly (readonly [string, string])[] = [['yes', '材料支持此表达／身份'], ['no', '本次材料未见'], ['unknown', '尚不能判定']];
const NEED_SUPPORT = '请选择至少一项当前材料作为支持依据。';

function isPositive(field: string, value: string): boolean {
  if (field === 'relevance') return value === 'related';
  if (field === 'focus') return value !== 'unknown';
  return value === 'yes';
}

interface Group {
  key: string;
  title: string;
  fragments: Fragment[];
}

function groupFragments(supports: Fragment[], workTitles: ReadonlyMap<string, string>): Group[] {
  const groups = new Map<string, Group>();
  for (const fragment of supports) {
    const key = fragment.field === 'biography' ? 'profile' : (fragment.workRef ?? 'work');
    let group = groups.get(key);
    if (!group) {
      const title = key === 'profile' ? '公开简介' : (fragment.workTitle ?? workTitles.get(key) ?? `作品材料 ${groups.size + 1}`);
      group = { key, title, fragments: [] };
      groups.set(key, group);
    }
    group.fragments.push(fragment);
  }
  return [...groups.values()];
}

export function CreatorCorrectionDialog({ target, onSaved, onClose }: CorrectionDialogProps) {
  const titleId = useId();
  const begin = useWriteGuard(`${target.creatorKey}:${target.workRef ?? ''}:${target.field}`);
  const write = useOverrideMutation();
  const inFlight = useRef(false);
  const choices = CHOICES[target.field] ?? TRAIT_CHOICES;
  const authorLevel = target.workRef === null;
  const [value, setValue] = useState(target.currentValue);
  const [reason, setReason] = useState(target.manual?.reason ?? '');
  const [selected, setSelected] = useState<ReadonlySet<string>>(() => new Set(target.manual?.supportFragmentIds ?? []));
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [busy, setBusy] = useState(false);
  const groups = groupFragments(target.supports, target.workTitles);
  const fieldName = target.field === 'focus' ? '作者倾向' : (FIELD_NAMES[target.field] ?? target.field);

  async function perform(next: string | null) {
    if (inFlight.current) return;
    const ids = next === null ? [] : [...selected];
    if (next !== null && isPositive(target.field, next) && ids.length === 0) {
      setFeedback({ tone: 'error', text: NEED_SUPPORT, code: null });
      return;
    }
    const isCurrent = begin();
    inFlight.current = true;
    setBusy(true);
    setFeedback(null);
    try {
      await write.mutateAsync({
        domain: target.domain,
        creatorKey: target.creatorKey,
        workRef: target.workRef,
        field: target.field,
        value: next,
        reason: next === null ? null : reason.trim() === '' ? null : reason.trim(),
        supportFragmentIds: ids,
      });
      if (isCurrent()) onSaved();
    } catch (error) {
      if (isCurrent()) setFeedback(feedbackForError(error, { retryNote: '这项判断按内容保存，重新保存同样的内容不会产生第二条记录。' }));
    } finally {
      inFlight.current = false;
      if (isCurrent()) setBusy(false);
    }
  }

  /** 用户改了判断或勾选后，「请选择支持材料」这条本地提示已经过时，撤掉它；服务器的回执不动。 */
  function dropStaleHint() {
    setFeedback(current => (current !== null && current.code === null && current.text === NEED_SUPPORT ? null : current));
  }

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    void perform(value);
  }

  return (
    <ModalDialog labelledBy={titleId} className={styles.dialog} onClose={onClose}>
      <form className={styles.form} onSubmit={submit}>
        <header className={styles.head}>
          <div>
            <small>仅调整指定字段</small>
            <h2 id={titleId}>
              {target.currentValue === 'unknown' ? '补充' : '调整'}
              {fieldName}
            </h2>
          </div>
          <button type="button" className={styles.textButton} onClick={onClose}>
            关闭
          </button>
        </header>
        <div className={styles.body}>
          <p>
            {authorLevel ? '创作者' : '作品'}：{target.subjectTitle} · {target.domainName} · 仅修改本项判断
          </p>
          <p className={styles.help}>判定依据：{CRITERIA[target.field] ?? ''}</p>
          <label>
            你的判断
            <select
              value={value}
              required
              onChange={event => {
                setValue(event.target.value);
                dropStaleHint();
              }}
            >
              {choices.map(([optionValue, label]) => (
                <option key={optionValue} value={optionValue}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <p className={styles.help}>保存后，这一项使用你的判断；自动分析不会覆盖。可随时恢复当前自动结果。</p>
          <fieldset>
            <legend>选择支持这项判断的材料</legend>
            {groups.length === 0 ? <p className={styles.help}>当前没有可选支持材料；仍可选择尚不能判定或恢复自动。</p> : null}
            {groups.map(group => (
              <details key={group.key} className={styles.supportGroup} open>
                <summary>
                  {group.title} · {group.fragments.length} 个片段
                </summary>
                {group.fragments.map(fragment => (
                  <label key={fragment.fragmentId}>
                    <input
                      type="checkbox"
                      checked={selected.has(fragment.fragmentId)}
                      onChange={event => {
                        const next = new Set(selected);
                        if (event.target.checked) next.add(fragment.fragmentId);
                        else next.delete(fragment.fragmentId);
                        setSelected(next);
                        dropStaleHint();
                      }}
                    />
                    <span className={styles.supportText}>
                      <small className={styles.supportKind}>{fieldSource(fragment.field)}</small>
                      {fragment.text || '片段暂不可读'}
                    </span>
                  </label>
                ))}
              </details>
            ))}
          </fieldset>
          <label>
            调整原因（可选）
            <textarea rows={3} maxLength={300} value={reason} onChange={event => setReason(event.target.value)} />
          </label>
        </div>
        <footer className={styles.foot}>
          <button type="button" className={styles.textButton} disabled={target.manual === null || busy} onClick={() => void perform(null)}>
            恢复自动判断
          </button>
          <div className={styles.footNote}>
            <FeedbackLine feedback={feedback} />
          </div>
          <button type="button" onClick={onClose}>
            取消
          </button>
          <button type="submit" className={styles.primary} disabled={busy}>
            {busy ? '保存中…' : '保存判断'}
          </button>
        </footer>
      </form>
    </ModalDialog>
  );
}
