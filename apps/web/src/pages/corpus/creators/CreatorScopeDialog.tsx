import { CREATOR_RESEARCH_GUIDANCE } from '../../settings/aiSettingsModel';
import { useId } from 'react';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { COEXISTENCE, CRITERIA, FIELD_NAMES } from './creatorsPresent';
import { formatCount, unknownAuthorLink, type LinkContext, type QuickKey } from './creatorsQuery';
import type { CreatorList } from './creatorsTypes';
import styles from './CreatorDialogs.module.css';

interface CreatorScopeDialogProps {
  data: CreatorList;
  usageRole: 'primary' | 'reference';
  hasDateRange: boolean;
  link: LinkContext;
  onQuick: (key: QuickKey) => void;
  onShowAllDates: () => void;
  onClose: () => void;
}

/** 当前研究状态与已有判断分开解释。 */
const STATE_MEANINGS: readonly (readonly [string, string])[] = [
  ['研究排队 / 执行中', '按本次固定的模型、提示词和 Skill 执行，状态以调用回执为准；尚未完成不代表无关或不具备。'],
  ['研究未完成', '调用失败、取消或条件不满足时保留原因；需核对当前材料和 AI 设置后显式重新提交，未知回执不得重复发送。'],
  ['已研究，依据不足', '已有材料不足以支持结论，判断仍是「尚不能判定」；不代表失败或没有。'],
  ['尚无研究结果', '当前没有可展示的研究结果；可在作品与作者详情检查底稿并显式发起研究。'],
];

/** 范围内的统计 + 领域说明 + 判断标准。统计只跟领域、用途、平台和发表日期变化，不随搜索与筛选改变。 */
export function CreatorScopeDialog({ data, usageRole, hasDateRange, link, onQuick, onShowAllDates, onClose }: CreatorScopeDialogProps) {
  const titleId = useId();
  const { stats, policy, domain, leadRule } = data;
  const hasRule = policy.likeThreshold !== null;
  const observationKnown = stats.outside !== null;
  const insideKnown = stats.inside !== null;
  const leadsKnown = observationKnown && leadRule !== null && stats.highLikeLeads !== null;
  const buttons: readonly { key: QuickKey; text: string; value: string; disabled: boolean }[] = [
    { key: 'all', text: '范围内作者', value: formatCount(stats.authors), disabled: false },
    { key: 'new', text: '最近新见', value: formatCount(stats.recentlyDiscovered), disabled: false },
    { key: 'vertical', text: '本领域持续创作', value: formatCount(stats.vertical), disabled: false },
    { key: 'personal', text: '有亲历自述作品', value: formatCount(stats.personal), disabled: false },
    { key: 'outside', text: '未加入观察', value: formatCount(stats.outside), disabled: !observationKnown },
    { key: 'inside', text: '已加入观察', value: formatCount(stats.inside), disabled: !insideKnown },
    {
      key: 'high',
      text: '高赞线索',
      value: formatCount(stats.highLikeLeads),
      disabled: !leadsKnown,
    },
  ];
  return (
    <ModalDialog labelledBy={titleId} className={styles.dialog} onClose={onClose} closeOnBackdrop>
      <div className={styles.form}>
        <header className={styles.head}>
          <div>
            <small>仅统计当前材料范围</small>
            <h2 id={titleId}>材料范围与判断标准</h2>
          </div>
          <button type="button" className={styles.textButton} onClick={onClose}>
            关闭
          </button>
        </header>
        <div className={styles.body}>
          <section aria-label="范围统计">
            <div className={styles.stats}>
              {buttons.map(button => (
                <button
                  key={button.key}
                  type="button"
                  disabled={button.disabled}
                  title="只查看此项，清除其他搜索与筛选"
                  onClick={() => onQuick(button.key)}
                >
                  {button.text}
                  <strong>{button.value}</strong>
                </button>
              ))}
            </div>
            <p className={styles.help}>
              统计按当前领域、用途和发表日期计算，不随搜索与筛选改变；列表上方的命中数才是当前搜索与筛选的结果。
              {leadRule
                ? `高赞线索：未加入观察，且同一作者至少 ${leadRule.minWorks} 篇作品点赞不少于 ${formatCount(leadRule.minLikes)}。`
                : '高赞线索口径暂不可用。'}
              {hasRule ? `相关爆款标准：点赞不少于 ${formatCount(policy.likeThreshold)}。` : '尚未设置爆款标准，爆款相关统计暂不可用。'}
            </p>
            <p className={styles.coverage}>
              当前范围 {formatCount(stats.works)} 篇作品 · 已确认相关 {formatCount(stats.relatedAuthors)} 位作者。
              <br />
              <a href={unknownAuthorLink(link)}>作者身份待补 {formatCount(stats.unknownAuthorWorks)} 篇</a>
              {hasDateRange && stats.unknownDateWorks > 0 ? (
                <>
                  {' · '}
                  <button type="button" className={styles.textButton} onClick={onShowAllDates}>
                    另有 {formatCount(stats.unknownDateWorks)} 篇发布时间未知
                  </button>
                </>
              ) : null}
            </p>
          </section>
          <section aria-label="领域说明">
            <h3>
              {domain.name ?? '当前领域'} · {usageRole === 'reference' ? '参照用途' : '主研究用途'}
            </h3>
            <p className={styles.help}>领域说明：{domain.description ?? '尚未填写'}</p>
            <p className={styles.help}>研究目标：{domain.researchGoal ?? '尚未填写'}</p>
            <p className={styles.help}>
              {CREATOR_RESEARCH_GUIDANCE}，当前标签仅统计已有判断。作品判断逐篇进行；账号判断只使用显式固定的作品样本和可用公开简介，
              实际取样篇数见各作者的依据，不要求全账号作品分析完。
            </p>
          </section>
          <section aria-label="判断标准">
            <h3>判断标准</h3>
            <dl className={styles.definitions}>
              {(['personal_experience', 'professional_output', 'explicit_promotion', 'institution_or_brand', 'focus', 'relevance'] as const).map(key => (
                <div key={key}>
                  <dt>{FIELD_NAMES[key]}</dt>
                  <dd>{CRITERIA[key]}</dd>
                </div>
              ))}
            </dl>
            <p className={styles.help}>{COEXISTENCE}</p>
          </section>
          <section aria-label="分析状态说明">
            <h3>分析状态的含义</h3>
            <dl className={styles.definitions}>
              {STATE_MEANINGS.map(([term, meaning]) => (
                <div key={term}>
                  <dt>{term}</dt>
                  <dd>{meaning}</dd>
                </div>
              ))}
            </dl>
            <p className={styles.help}>未知与零分别展示：未知显示「—」，不会变成 0。</p>
          </section>
        </div>
      </div>
    </ModalDialog>
  );
}
