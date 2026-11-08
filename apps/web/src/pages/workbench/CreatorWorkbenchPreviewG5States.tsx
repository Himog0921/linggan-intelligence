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
  { id: 'synthetic-exp-1', title: '一次改变安排的经历', body: '原来计划很满，后来只保留了当天最重要的两件事。', confirmed: true },
  { id: 'synthetic-exp-2', title: '一次尝试新方法的经历', body: '把一段很复杂的任务拆成了几步，先从第一步开始。', confirmed: false },
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
          <small>CREATOR WORKBENCH · ISOLATED PREVIEW</small>
          <h2>{kind === 'experience-picker' ? '选一段经历' : kind === 'save-as-template' ? '存为样板' : '创作使用路径'}</h2>
        </div>
        <ActionButton aria-label="关闭预览对话框" variant="outline" onClick={onClose}>关闭</ActionButton>
      </header>
      {kind === 'experience-picker' ? (
        <div className={styles.content}>
          <p className={styles.subtle}>从本人已确认的经历中选择。未确认记录不可采用；这里是合成数据，只写入本地演示稿件。</p>
          <div className={styles.cards}>
            {EXPERIENCE_OPTIONS.map(item => (
              <article key={item.id} className={styles.card}>
                <div className={styles.cardHead}><strong>{item.title}</strong><small>{item.confirmed ? '已确认' : '待本人确认'}</small></div>
                <p>{item.body}</p>
                <ActionButton variant="outline" disabled={!item.confirmed} onClick={() => { onUseExperience(item.body); onClose(); }}>
                  {item.confirmed ? '用这段' : '尚不可用'}
                </ActionButton>
              </article>
            ))}
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
