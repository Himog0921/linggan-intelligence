import { render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';
import { ImageCompare } from './ImageCompare';
import { parseDossierPayload } from './dossierDecode';
import { dossierJson, dossierMediaJson } from './evidenceFixtures';

it('原始 OCR、部分采用和待核验保持独立，空采用不显示为没有文字', () => {
  const raw = { ...dossierMediaJson(1), researchText: '', displayText: '', ocrAdoption: {
    rawBlocks: [{ blockRef: 'b', text: '合成原文尚待核验' }], pendingBlocks: ['b'], projectionVersion: 'ocr-source-gaps-v1',
  } };
  const media = parseDossierPayload(dossierJson(1, { media: [raw] })).dossier.media[0]!;
  render(<ImageCompare media={media} register={null} onOpenImage={() => {}} />);
  expect(screen.getByText('尚无可直接采用的文字，请查看待核验片段。')).toBeInTheDocument();
  expect(screen.getByText('待核验片段（1 个区块）')).toBeInTheDocument();
  expect(screen.getByText('原始 OCR（未筛选）')).toBeInTheDocument();
  expect(screen.getAllByText('合成原文尚待核验')).toHaveLength(2);
});
