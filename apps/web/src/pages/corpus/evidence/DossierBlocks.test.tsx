import { render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';
import { DossierBlocks } from './DossierBlocks';
import { parseDossierPayload } from './dossierDecode';
import { dossierJson, dossierMaterialJson } from './evidenceFixtures';

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
