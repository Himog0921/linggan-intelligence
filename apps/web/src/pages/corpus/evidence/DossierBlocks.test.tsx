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

it('图片原文与模型视觉描述分开展示并保留区域归属', () => {
  const unit = parseDossierPayload(dossierJson(1, { materials: [dossierMaterialJson(1, { blocks: [
    { ref: 'text', kind: 'paragraph', text: '不适合三岁以下', speaker: 'unconfirmed', contentType: 'extracted_text', regionRef: 'r1', regionLabel: '左侧产品说明' },
    { ref: 'visual', kind: 'paragraph', text: '红色收纳盒，旁边有卡片', speaker: 'unconfirmed', contentType: 'visual_description', regionRef: 'r1', regionLabel: '左侧产品说明' },
  ] })] })).dossier.materials[0]!;
  render(<DossierBlocks blocks={unit.blocks}/>);
  expect(screen.getByText('图片文字 · 识别提取')).toBeVisible();
  expect(screen.getByText('视觉描述 · 模型观察，非原文')).toBeVisible();
  expect(screen.getByText('不适合三岁以下')).toBeVisible();
  expect(screen.getByText('红色收纳盒，旁边有卡片')).toBeVisible();
});
