import { useEffect, useId, useRef, useState, type FormEvent } from 'react';
import { useBlocker } from 'react-router';
import { ActionButton } from '../../../shared/ActionButton';
import { CreatorSelect } from './CreatorSelect';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { COEXISTENCE, CRITERIA } from './creatorsPresent';
import { DEFAULT_QUERY, LABELS, TRAIT_KEYS, dateRangeError, presetDates, type CreatorQuery } from './creatorsQuery';
import styles from './CreatorDialogs.module.css';

interface CreatorFilterDialogProps {
  /** 已应用的条件。对话框里的修改只是草稿，点「应用筛选」才生效；取消不改任何东西。 */
  query: CreatorQuery;
  /** 配置了爆款标准才能勾「有相关爆款」。 */
  hasThreshold: boolean;
  /** 观察关系读得到才能按观察关系筛选；读不到时不能把全体算成「未纳入」。 */
  observationAvailable: boolean;
  onApply: (next: CreatorQuery) => void;
  onClose: () => void;
}

type WindowKind = 'all' | '30' | '90' | 'custom';

interface Draft {
  usageRole: CreatorQuery['usageRole'];
  window: WindowKind;
  publishedFrom: string;
  publishedTo: string;
  traits: string[];
  focus: string;
  observation: string;
  relevance: string;
  minLikes: string;
  topicHint: string;
  viral: boolean;
  highLikes: boolean;
  /** 搜索词与排序不在这个对话框里编辑，但「清除额外条件」会一并清掉它们。 */
  query: string;
  sort: CreatorQuery['sort'];
}

function draftFrom(query: CreatorQuery): Draft {
  return {
    usageRole: query.usageRole,
    window: query.publishedFrom || query.publishedTo ? 'custom' : 'all',
    publishedFrom: query.publishedFrom,
    publishedTo: query.publishedTo,
    traits: query.traits,
    focus: query.focus,
    observation: query.observation,
    relevance: query.relevance,
    minLikes: query.minLikes === null ? '' : String(query.minLikes),
    topicHint: query.topicHint,
    viral: query.viral,
    highLikes: query.highLikes,
    query: query.query,
    sort: query.sort,
  };
}

function applyDraft(base: CreatorQuery, draft: Draft): CreatorQuery {
  const minLikes = draft.minLikes.trim();
  return {
    ...base,
    usageRole: draft.usageRole,
    publishedFrom: draft.publishedFrom,
    publishedTo: draft.publishedTo,
    traits: TRAIT_KEYS.filter(key => draft.traits.includes(key)),
    focus: draft.focus,
    observation: draft.observation,
    relevance: draft.relevance,
    minLikes: minLikes === '' ? null : Number(minLikes),
    topicHint: draft.topicHint.trim(),
    viral: draft.viral,
    highLikes: draft.highLikes,
    query: draft.query,
    sort: draft.sort,
    page: 0,
  };
}

export function CreatorFilterDialog({ query, hasThreshold, observationAvailable, onApply, onClose }: CreatorFilterDialogProps) {
  const titleId = useId();
  const [draft, setDraft] = useState<Draft>(() => draftFrom(query));
  const [error, setError] = useState<string | null>(null);
  const update = (patch: Partial<Draft>) => setDraft(current => ({ ...current, ...patch }));

  const dirty = JSON.stringify(draft) !== JSON.stringify(draftFrom(query));
  const [closing, setClosing] = useState(false);
  const applying = useRef(false);
  const blocker = useBlocker(() => dirty && !applying.current);
  const confirming = closing || blocker.state === 'blocked';
  function requestClose() { if (dirty) setClosing(true); else onClose(); }
  function keepEditing() { setClosing(false); if (blocker.state === 'blocked') blocker.reset(); }
  function discard() { if (blocker.state === 'blocked') blocker.proceed(); else onClose(); }
  useEffect(() => {
    if (!dirty) return;
    const warn = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ''; };
    window.addEventListener('beforeunload', warn);
    return () => window.removeEventListener('beforeunload', warn);
  }, [dirty]);

  function chooseWindow(window: WindowKind) {
    if (window === 'custom') return update({ window });
    if (window === 'all') return update({ window, publishedFrom: '', publishedTo: '' });
    const { from, to } = presetDates(Number(window), new Date());
    update({ window, publishedFrom: from, publishedTo: to });
  }

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!event.currentTarget.reportValidity()) return;
    const problem = dateRangeError(draft.publishedFrom, draft.publishedTo);
    if (problem) {
      setError(problem);
      return;
    }
    applying.current = true;
    onApply(applyDraft(query, draft));
  }

  return (
    <ModalDialog labelledBy={titleId} className={styles.dialog} onClose={requestClose} closeOnBackdrop>
      <form className={styles.form} onSubmit={submit}>
        <header className={styles.head}>
          <div>
            <small>创作者 · 查询条件</small>
            <h2 id={titleId}>筛选材料与作者</h2>
          </div>
          <button type="button" className={styles.textButton} aria-label="关闭筛选" onClick={requestClose}>
            关闭
          </button>
        </header>
        <div className={styles.body}>
          <section>
            <h3>材料范围</h3>
            <div className={styles.grid}>
              <div className={styles.selectField}>
                <span>材料用途</span>
                <CreatorSelect label="材料用途" value={draft.usageRole} onChange={value => update({ usageRole: value === 'reference' ? 'reference' : 'primary' })}
                  options={[{ value: 'primary', label: '主研究' }, { value: 'reference', label: '参照' }]} />
              </div>
              <div className={styles.selectField}><span>平台</span>
                <CreatorSelect label="平台" value="xhs" disabled onChange={() => undefined} options={[{ value: 'xhs', label: '小红书' }]} />
              </div>
              <div className={styles.selectField}>
                <span>作品发表日期</span>
                <CreatorSelect label="作品发表日期" value={draft.window} onChange={value => chooseWindow(value as WindowKind)}
                  options={[{ value: 'all', label: '全部日期' }, { value: '30', label: '近30天' }, { value: '90', label: '近90天' }, { value: 'custom', label: '自定义日期' }]} />
              </div>
              {draft.window === 'custom' ? (
                <div className={`${styles.grid} ${styles.full}`}>
                  <label>
                    开始日期
                    <input type="date" value={draft.publishedFrom} onChange={event => update({ publishedFrom: event.target.value })} />
                  </label>
                  <label>
                    结束日期（不含）
                    <input type="date" value={draft.publishedTo} onChange={event => update({ publishedTo: event.target.value })} />
                  </label>
                </div>
              ) : null}
            </div>
          </section>
          <section>
            <h3>作品表达（作品层面）</h3>
            <p className={styles.help}>组内任选其一；不同条件取交集。作品条件在同一篇作品上核对。机构／品牌自述根据公开简介判断。</p>
            <div className={styles.checks}>
              {TRAIT_KEYS.map(key => (
                <label key={key}>
                  <input
                    type="checkbox"
                    checked={draft.traits.includes(key)}
                    onChange={event => update({ traits: event.target.checked ? [...draft.traits, key] : draft.traits.filter(item => item !== key) })}
                  />
                  {LABELS[key]}
                </label>
              ))}
            </div>
            <p className={styles.help}>{CRITERIA.personal_experience}</p>
          </section>
          <section>
            <h3>作者倾向与作品条件</h3>
            <div className={styles.grid}>
              <div className={styles.selectField}>
                <span>作者倾向（作者层面）</span>
                <CreatorSelect label="作者倾向（作者层面）" value={draft.focus} onChange={value => update({ focus: value })}
                  options={[{ value: '', label: '全部' }, { value: 'vertical_tendency', label: '本领域持续创作' }, { value: 'multi_topic', label: '多主题分享' }, { value: 'unknown', label: '尚不能判定' }]} />
              </div>
              <div className={styles.selectField}>
                <span>观察关系</span>
                <CreatorSelect label="观察关系" value={draft.observation} disabled={!observationAvailable} onChange={value => update({ observation: value })}
                  options={[{ value: '', label: '全部' }, { value: 'outside', label: '未加入本领域' }, { value: 'inside', label: '已加入本领域' }, { value: 'monitoring', label: '巡查中' }, { value: 'paused', label: '已暂停' }, { value: 'other_domains_only', label: '仅其他领域有目标' }, { value: 'dismissed', label: '已忽略' }]} />
              </div>
              <div className={styles.selectField}>
                <span>领域相关性</span>
                <CreatorSelect label="领域相关性" value={draft.relevance} onChange={value => update({ relevance: value })}
                  options={[{ value: '', label: '全部' }, { value: 'related', label: '已判相关' }, { value: 'unknown', label: '待判断' }, { value: 'unrelated', label: '已判无关' }]} />
              </div>
              <label>
                最低点赞
                <input type="number" min={0} step={1} placeholder="不限" value={draft.minLikes} onChange={event => update({ minLikes: event.target.value })} />
              </label>
              <label className={styles.full}>
                内容方向短语（精确匹配）
                <input type="search" maxLength={120} placeholder="输入已有分析短语" value={draft.topicHint} onChange={event => update({ topicHint: event.target.value })} />
              </label>
            </div>
            <div className={styles.checks}>
              <label>
                <input type="checkbox" checked={draft.viral} disabled={!hasThreshold} onChange={event => update({ viral: event.target.checked })} />
                有相关爆款
              </label>
              <label>
                <input type="checkbox" checked={draft.highLikes} onChange={event => update({ highLikes: event.target.checked })} />
                有已知点赞的作品
              </label>
            </div>
            <p className={styles.help}>{hasThreshold ? '最低点赞只改变当前筛选，不修改爆款标准。' : '尚未设置爆款标准；仍可筛出有已知点赞的作品。'}</p>
            <p className={styles.help}>{CRITERIA.focus}</p>
            <p className={styles.help}>{COEXISTENCE}</p>
            {error ? (
              <p role="alert" className={styles.inlineError}>
                {error}
              </p>
            ) : null}
          </section>
        </div>
        <footer className={styles.foot}>
          {confirming ? <>
            <p className={styles.footNote} role="status">筛选尚未应用，放弃本次修改？</p>
            <ActionButton autoFocus onClick={keepEditing}>继续筛选</ActionButton>
            <ActionButton onClick={discard}>放弃修改</ActionButton>
          </> : <>
          <button type="button" className={styles.textButton}
            onClick={() => setDraft(current => ({ ...current, traits: [], focus: '', observation: '', relevance: '', minLikes: '', topicHint: '', viral: false, highLikes: false, query: '', sort: DEFAULT_QUERY.sort }))}>
            清除额外条件
          </button>
          <span className={styles.spacer} />
          <ActionButton onClick={requestClose}>取消</ActionButton>
          <ActionButton type="submit" variant="primary">应用筛选</ActionButton>
          </>}
        </footer>
      </form>
    </ModalDialog>
  );
}
