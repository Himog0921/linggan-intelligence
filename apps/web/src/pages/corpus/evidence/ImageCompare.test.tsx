import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';
import { ImageCompare } from './ImageCompare';
import { DossierBlocks } from './DossierBlocks';
import { parseDossierPayload } from './dossierDecode';
import { dossierJson, dossierMaterialJson } from './evidenceFixtures';
it('真实 [x,y,width,height] 定位；无坐标不编造框，原图失败清除框', () => {
  const material = parseDossierPayload(dossierJson(1)).dossier.materials[1]!;
  const { container } = render(<ImageCompare material={material} register={null} onOpenImage={() => {}} />);
  fireEvent.click(screen.getByText(/对照原图/)); fireEvent.click(screen.getByRole('button', { name: /1 · 封面大字/ }));
  expect(container.querySelector('span')?.getAttribute('style')).toBe('left: 10%; top: 10%; width: 75%; height: 7.000000000000001%;');
  fireEvent.error(screen.getByRole('img')); expect(container.querySelector('span')).toBeNull(); expect(screen.getByText(/本地原图暂时不可读/)).toBeInTheDocument();
});
it('完整长文与表格、对话直接可读，未知表达者不变成作者', () => {
  const long = '不能忽略这一段。'.repeat(12000);
  const raw = dossierMaterialJson(1, { blocks: [
    { ref: 'long', kind: 'paragraph', text: long, speaker: 'unconfirmed' },
    { ref: 'list', kind: 'list', ordered: true, start: 4, items: ['第一项','第二项'] },
    { ref: 'table', kind: 'table', columns: ['内容','数值'], rows: [['负数','-0.25']] },
    { ref: 'talk', kind: 'dialogue', turns: [{ speaker: 'other', text: '他人的说法' }, { speaker: 'author', text: '作者回应' }] },
  ] }); const unit = parseDossierPayload(dossierJson(1, { materials: [raw] })).dossier.materials[0]!;
  const { container } = render(<DossierBlocks blocks={unit.blocks} />);
  expect(screen.getByText(long)).toBeVisible(); expect(container.querySelector('details')).toBeNull(); expect(screen.getByRole('table')).toHaveTextContent('-0.25'); expect(container.querySelector('ol')).toHaveAttribute('start', '4'); expect(screen.queryByText('表达者未确认')).not.toBeInTheDocument(); expect(container.querySelector('[data-block-ref="long"]')).toHaveAttribute('data-speaker', 'unconfirmed'); expect(screen.getByText('他人的说法')).toBeVisible();
});
