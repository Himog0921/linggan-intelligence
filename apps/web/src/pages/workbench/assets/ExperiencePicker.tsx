import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import type { WorkDocument } from '../workbenchModel';
import * as api from './assetsApi';
import { insertExperience, usableExperiences } from './assetsText';
import styles from './Assets.module.css';
import { SelectField } from '../../../shared/SelectField';
import { FieldLabel } from '../FieldLabel';

export function ExperiencePicker({ document, edit }: { document: WorkDocument; edit: (doc: WorkDocument) => void }) {
  const [open, setOpen] = useState(false);
  const [chosen, setChosen] = useState('');
  const items = useQuery({ queryKey: ['creation-assets', 'experiences', document.accountRef],
    queryFn: ({ signal }) => api.listExperiences(document.accountRef!, signal), enabled: open && !!document.accountRef, gcTime: 0 });
  const available = usableExperiences(document, items.data ?? []);
  const experience = available.find(item => item.id === chosen);
  function insert() {
    const next = experience ? insertExperience(document, experience) : null;
    if (!next) return;
    edit(next);
    setOpen(false); setChosen('');
  }
  return <div className={styles.slot}>
    <div className={styles.actions}><ActionButton variant="outline" onClick={() => setOpen(value => !value)}>{open ? '收起经历库' : '从经历库选'}</ActionButton>
      <ActionButton variant="quiet" onClick={() => { const id = crypto.randomUUID(); edit({ ...document, paragraphs: [...document.paragraphs, { id, text: '' }], experienceGaps: [...document.experienceGaps, id] }); }}>留一个经历位</ActionButton></div>
    {open ? !document.accountRef ? <p className={styles.hint}>请先为这篇稿件选择账号。</p> : <>
      {items.isPending ? <p role="status">正在读取经历…</p> : items.isError ? <p role="alert" className={styles.problem}>经历读取失败，请重试。</p> : null}
      {items.isSuccess && available.length === 0 ? <p className={styles.hint}>没有可新增的本人确认经历；已放入本稿的同一版本不重复插入。</p> : null}
      {available.length ? <><FieldLabel>选择本人已确认的经历<SelectField label="选择本人已确认的经历" value={chosen} options={[{ value: '', label: '选择经历' }, ...available.map(item => ({ value: item.id, label: `${item.code} · ${item.title}` }))]} onChange={setChosen} /></FieldLabel>
        <ActionButton disabled={!experience} onClick={insert}>放入正文</ActionButton></> : null}
    </> : null}
  </div>;
}
