import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import type { WorkDocument } from '../workbenchModel';
import * as api from './assetsApi';
import styles from './Assets.module.css';

export function ExperiencePicker({ document, edit }: { document: WorkDocument; edit: (doc: WorkDocument) => void }) {
  const [open, setOpen] = useState(false);
  const [chosen, setChosen] = useState('');
  const items = useQuery({ queryKey: ['creation-assets', 'experiences', document.accountRef],
    queryFn: ({ signal }) => api.listExperiences(document.accountRef!, signal), enabled: open && !!document.accountRef, gcTime: 0 });
  const available = (items.data ?? []).filter(item => item.currentPerson && item.state === 'confirmed' && item.body !== null &&
    !document.experienceSlots.some(slot => slot.experienceRef === item.id && slot.experienceRevision === item.revision));
  const experience = available.find(item => item.id === chosen);
  function insert() {
    if (!experience?.body) return;
    const gap = document.experienceGaps[0];
    const paragraphId = gap ?? crypto.randomUUID();
    edit({ ...document,
      paragraphs: gap ? document.paragraphs.map(p => p.id === gap ? { ...p, text: experience.body! } : p)
        : [...document.paragraphs, { id: paragraphId, text: experience.body }],
      experienceSlots: [...document.experienceSlots, { paragraphId, experienceRef: experience.id, experienceRevision: experience.revision }],
      experienceGaps: document.experienceGaps.filter(id => id !== paragraphId),
    });
    setOpen(false); setChosen('');
  }
  return <div className={styles.slot}>
    <div className={styles.actions}><ActionButton variant="outline" onClick={() => setOpen(value => !value)}>{open ? '收起经历库' : '从经历库选'}</ActionButton>
      <ActionButton variant="quiet" onClick={() => { const id = crypto.randomUUID(); edit({ ...document, paragraphs: [...document.paragraphs, { id, text: '' }], experienceGaps: [...document.experienceGaps, id] }); }}>留一个经历位</ActionButton></div>
    {open ? !document.accountRef ? <p className={styles.hint}>请先为这篇稿件选择账号。</p> : <>
      {items.isPending ? <p role="status">正在读取经历…</p> : items.isError ? <p role="alert" className={styles.problem}>经历读取失败，请重试。</p> : null}
      {items.isSuccess && available.length === 0 ? <p className={styles.hint}>没有可新增的本人确认经历；已放入本稿的同一版本不重复插入。</p> : null}
      {available.length ? <><label>选择本人已确认的经历<select value={chosen} onChange={e => setChosen(e.target.value)}><option value="">选择经历</option>
        {available.map(item => <option key={item.id} value={item.id}>{item.code} · {item.title}</option>)}</select></label>
        <ActionButton disabled={!experience} onClick={insert}>放入正文</ActionButton></> : null}
    </> : null}
  </div>;
}
