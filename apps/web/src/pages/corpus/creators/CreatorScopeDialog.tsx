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

/** 分析状态的四种含义，彼此不能互相替代（#384 发现）。 */
const STATE_MEANINGS: readonly (readonly [string, string])[] = [
  ['排队待分析 / 分析中', '任务在路上，结果还没有出来；不代表已经判为无关或不具备。'],
  ['分析失败 / 已暂停', '执行没有完成（原因见各作者的处理信息），可以在依据抽屉里重试失败的分析；不代表材料有问题。'],
  ['已分析，依据不足', '分析已经做过，但已取得的材料撑不起结论，所以判断仍是「尚不能判定」；不是失败，也不是没有。'],
  ['尚未取得当前分析结果 / 自动分析未开启', '还没有当前的分析结果，或本领域没有开启自动分析；页面只展示已有判断，浏览与切换领域不会启动分析。'],
];

/** 范围内的统计 + 领域说明 + 判断标准。统计只跟领域、用途、平台和发表日期变化，不随搜索与筛选改变。 */
export function CreatorScopeDialog({ data, usageRole, hasDateRange, link, onQuick, onShowAllDates, onClose }: CreatorScopeDialogProps) {
  const titleId = useId();
  const { stats, policy, domain } = data;
  const hasRule = policy.likeThreshold !== null;
  const observationKnown = stats.outside !== null;
  const buttons: readonly { key: QuickKey; text: string; value: string; disabled: boolean }[] = [
    { key: 'all', text: '范围内作者', value: formatCount(stats.authors), disabled: false },
    { key: 'vertical', text: '本领域持续创作', value: formatCount(stats.vertical), disabled: false },
    { key: 'personal', text: '有亲历自述作品', value: formatCount(stats.personal), disabled: false },
    { key: 'outside', text: '未加入观察', value: formatCount(stats.outside), disabled: !observationKnown },
    {
      key: 'high',
      text: hasRule ? '未观察的爆款作者' : '未观察的高赞作者',
      value: hasRule ? formatCount(stats.outsideViral) : '查看 →',
      disabled: !observationKnown,
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
              {hasRule ? `相关爆款标准：点赞不少于 ${formatCount(policy.likeThreshold)}。` : '尚未设置爆款标准，高赞线索仅按已有点赞排序。'}
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
              自动分析：{policy.analysisEnabled ? '本领域已开启' : '本领域未开启，当前标签仅统计已有判断'}。作品判断逐篇进行；作者判断只取样本领域已有作品的摘要，
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
