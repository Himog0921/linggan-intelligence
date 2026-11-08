import { useEffect, useRef, useState } from 'react';
import { ActionButton } from '../../shared/ActionButton';
import styles from './CreatorWorkbenchPreviewG5States.module.css';

export type PreviewDialogKind = 'experience-picker' | 'save-as-template' | 'path-guide' | null;

const GUIDED_STEPS = [
  ['找选题', '从可追溯材料里挑出具体角度'],
  ['定选题', '整理选题框、材料和准备发给哪些账号'],
  ['起稿', '从选题框生成本地演示稿件'],
  ['改稿', '逐段编辑并检查标题、身份与素材'],
  ['发出', '通过正式流程复制，再由外部平台发布'],
  ['回收', '发布记录与后续数据需要正式业务接入'],
] as const;
const EXPERIENCE_OPTIONS = [
  { id: 'JL-0001', title: '放学后一起整理书包', body: '刚上小学那会，他总忘记整理东西。我一放学就陪他一起检查书本，逐项确认。', confirmed: true, used: true },
  { id: 'JL-0002', title: '上学前列好物品清单', body: '上学前我发现他总丢三落四，立刻列好每天要带的清单，生怕第二天忘记。', confirmed: true, used: true },
  { id: 'JL-0003', title: '每周固定整理一次', body: '后来我试过不少办法，提醒、检查、再提醒，基本每件事都要我跟着做。', confirmed: true, used: true },
  { id: 'JL-0004', title: '我去做自己的事，他反倒写了一段', body: '他写作业时我总站在旁边，一走神我马上喊。我去做自己的事，过一会回来看，他反倒写完一段。', confirmed: true, used: true },
  { id: 'JL-0005', title: '把周末安排换成公园散步', body: '周末我们把原来的安排换成去公园走走，不聊任务以后，家里的气氛轻松了很多。', confirmed: true, used: true },
  { id: 'JL-0006', title: '开始主动准备出行物品', body: '他开始主动整理出行物品，也愿意和家人讨论第二天要带什么。', confirmed: true, used: true },
  { id: 'JL-0007', title: '第一次自己定闹钟起床（示例）', body: '上周他第一次自己定好闹钟，早上按时起床。', confirmed: false, used: false },
] as const;

export function CreatorWorkbenchPreviewG5States({
  kind, onClose, draftTitle, paragraphRoles, onUseExperience, onPreviewOnly,
}: {
  kind: PreviewDialogKind;
  onClose: () => void;
  draftTitle: string;
  paragraphRoles: string[];
  onUseExperience: (body: string) => void;
  onPreviewOnly: (action: string) => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const [sampleName, setSampleName] = useState('');
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (kind && !dialog.open) dialog.showModal();
    if (!kind && dialog.open) dialog.close();
  }, [kind]);

  return (
    <dialog
      ref={ref}
      className={styles.dialog}
      data-preview-dialog={kind ?? undefined}
      aria-label={kind === 'experience-picker' ? '选择经历' : kind === 'save-as-template' ? '存为样板' : '使用路径'}
      onCancel={event => { event.preventDefault(); onClose(); }}
      onClick={event => { if (event.target === event.currentTarget) onClose(); }}
    >
      <header className={styles.header}>
        <div>
          {kind !== 'experience-picker' ? <small>CREATOR WORKBENCH · ISOLATED PREVIEW</small> : null}
          <h2>{kind === 'experience-picker' ? '从经历库选 · 示例妈妈' : kind === 'save-as-template' ? '存为样板' : '创作使用路径'}</h2>
        </div>
        <ActionButton aria-label="关闭预览对话框" variant="outline" onClick={onClose}>{kind === 'experience-picker' ? '×' : '关闭'}</ActionButton>
      </header>
      {kind === 'experience-picker' ? (
        <div className={styles.experienceContent}>
          <p className={styles.subtle}>只列出示例妈妈自己的经历；待本人确认的还不能用。以下均为合成演示内容。</p>
          <div className={styles.experienceRows}>
            {EXPERIENCE_OPTIONS.map(item => (
              <article key={item.id} className={styles.experienceRow}>
                <div className={styles.experienceRowHead}>
                  <strong><small>{item.id}</small> {item.title}</strong>
                  <span>{item.confirmed ? '已确认' : '待本人确认'}</span>
                </div>
                <p>{item.body}</p>
                <div className={styles.experienceRowFoot}>
                  <span className={item.confirmed ? styles.confirmed : styles.pending}>{item.confirmed ? '已确认' : '待本人确认'}</span>
                  {item.used ? <small>本篇已用</small> : null}
                  <ActionButton variant="outline" disabled={!item.confirmed} onClick={() => { onUseExperience(item.body); onClose(); }}>
                    {item.confirmed ? '用这段' : '尚不可用'}
                  </ActionButton>
                </div>
              </article>
            ))}
          </div>
          <div className={styles.experienceFooter}>
            <ActionButton variant="outline" onClick={() => onPreviewOnly('没有合适的，去采集')}>没有合适的，去采集</ActionButton>
          </div>
        </div>
      ) : null}
      {kind === 'save-as-template' ? (
        <div className={styles.content}>
          <label className={styles.label}>样板名称
            <input value={sampleName} onChange={event => setSampleName(event.target.value)}
              placeholder={draftTitle} aria-label="样板名称" />
          </label>
          <p className={styles.subtle}>提取稿件结构供审查。不会写入正式样板库，也不生成真实审核回执。</p>
          <div className={styles.steps}>
            {paragraphRoles.map((role, index) => <div key={index}><small>{String(index + 1).padStart(2, '0')}</small><strong>{role}</strong><span>从当前演示稿件结构提取</span></div>)}
          </div>
          <div className={styles.footer}>
            <ActionButton variant="quiet" onClick={onClose}>取消</ActionButton>
            <ActionButton variant="primary" onClick={() => { onPreviewOnly('存进样板库'); onClose(); }}>存进样板库（仅预览）</ActionButton>
          </div>
        </div>
      ) : null}
      {kind === 'path-guide' ? (
        <div className={styles.content}>
          <p className={styles.subtle}>从研究材料到创作、发布和数据回收。当前仅展示 UI 路径，尚未接入真实业务服务。</p>
          <div className={styles.steps}>
            {GUIDED_STEPS.map(([label, description], index) => (
              <div key={label}><small>{String(index + 1).padStart(2, '0')}</small><strong>{label}</strong><span>{description}</span></div>
            ))}
          </div>
          <h3>四条常走的路</h3>
          <div className={styles.cards}>
            {['今日成品包 → 改稿 → 发出 → 回收', '找材料 → 定选题 → 一题多号开稿', '证据库 → 提取结构 → 样板审核', '发布数据 → 复盘 → 再次选题'].map(route => <div key={route} className={styles.card}>{route}</div>)}
          </div>
          <div className={styles.footer}><ActionButton variant="primary" onClick={onClose}>知道了</ActionButton></div>
        </div>
      ) : null}
    </dialog>
  );
}

export function CreatorWorkbenchPreviewTour({ onClose }: { onClose: () => void }) {
  const checklist = [
    '确认账号身份与内容边界',
    '检查必改项目后再考虑复制',
    '从写稿进入材料桌，再返回原稿件',
    '选材料，写一句思路',
    '为一个选题挑选多个账号',
    '在右栏查看标题建议',
    '到资产库查看样板的审核状态',
    '选择本人已确认的经历',
  ];
  return <section className={styles.tour} aria-label="体验清单">
    <header className={styles.cardHead}><strong>体验清单</strong><ActionButton variant="quiet" onClick={onClose}>收起</ActionButton></header>
    <p className={styles.subtle}>按步骤体验预览；不会产生正式业务写入。</p>
    <ol>{checklist.map(item => <li key={item}>{item}</li>)}</ol>
  </section>;
}
