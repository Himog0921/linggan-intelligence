import { describe, expect, it } from 'vitest';
import { newDocument, parseWorkDocument, parseSaveReceipt, parseSourceList, scalarRange, WorkbenchContractError } from './workbenchModel';
const id = '00000000-0000-4000-8000-000000000001';
describe('closed workbench boundary', () => {
  it('keeps exact Chinese emoji/newline scalar offsets without normalization', () => {
    expect(scalarRange('甲😀\n乙  ', 1, 5)).toEqual({
      start: 1,
      end: 4,
      text: '😀\n乙'
    });
  });
  it('rejects unknown document fields, malformed identities and non-finite crops', () => {
    expect(() => parseWorkDocument({
      ...newDocument(),
      unexpected: true
    })).toThrow(WorkbenchContractError);
    expect(() => parseWorkDocument({
      ...newDocument(),
      paragraphs: [{
        id: 'not-a-uuid',
        text: '文字'
      }]
    })).toThrow(WorkbenchContractError);
  });
  it('preserves server-derived quote provenance separately from expression', () => {
    const document = {
      ...newDocument(),
      paragraphs: [{
        id,
        text: '自己的句子，原文😀。'
      }],
      uses: [{
        id,
        canvasId: id,
        expressionRevision: 1,
        excerptId: id,
        target: {
          kind: 'paragraph',
          paragraphId: id
        },
        placedTextHash: 'h',
        state: 'modified',
        contentOrigin: 'excerpt',
        sourceSpan: {
          start: 6,
          end: 9,
          textHash: 'hash'
        }
      }]
    };
    expect(parseWorkDocument(document).uses[0]?.sourceSpan).toEqual({
      start: 6,
      end: 9,
      textHash: 'hash'
    });
  });
  it('does not turn an unknown count into zero or accept a future response shape', () => {
    expect(() => parseSourceList({
      contract: 'content-workbench-sources.v1',
      total: null
    })).toThrow(WorkbenchContractError);
    expect(() => parseSaveReceipt({
      requestId: id,
      workId: id,
      outcome: 'accepted',
      replayed: false,
      revision: {
        id,
        number: 1,
        savedAt: 'now',
        document: {
          ...newDocument(),
          title: '已保存'
        }
      },
      future: 'ignored'
    })).toThrow(WorkbenchContractError);
  });
});
it('creates both source-backed and personal cards at the server-supported initial expression revision', async () => {
  const {
    newCanvasBlock
  } = await import('./workbenchModel');
  expect(newCanvasBlock().expressionRevision).toBe(1);
  expect(newCanvasBlock('opening', id).expressionRevision).toBe(1);
});
describe('server-derived authored metadata provenance targets', () => {
  it.each([{
    kind: 'angle'
  }, {
    kind: 'outline',
    index: 0
  }, {
    kind: 'tag',
    index: 2
  }, {
    kind: 'canvasNote',
    canvasId: id
  }, {
    kind: 'canvasExpression',
    canvasId: id
  }])('preserves %j with a closed target schema', async target => {
    const {
      parseUseTarget
    } = await import('./workbenchModel');
    expect(parseUseTarget(target)).toEqual(target);
  });
  it.each([{
    kind: 'futureTarget'
  }, {
    kind: 'outline',
    index: -1
  }, {
    kind: 'tag',
    index: 0.5
  }, {
    kind: 'angle',
    index: 0
  }, {
    kind: 'canvasNote',
    canvasId: 'invalid'
  }, {
    kind: 'canvasExpression',
    canvasId: id,
    expression: 'untrusted'
  }])('rejects unsupported/invalid target %j', async target => {
    const {
      parseUseTarget
    } = await import('./workbenchModel');
    expect(() => parseUseTarget(target)).toThrow(WorkbenchContractError);
  });
});
