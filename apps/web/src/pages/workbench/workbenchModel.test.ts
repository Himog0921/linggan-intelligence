import { describe, expect, it } from 'vitest';
import { newDocument, parseWorkDocument, parseSaveReceipt, parseSourceList, parseAiStatus, scalarRange, WorkbenchContractError } from './workbenchModel';
const id = '00000000-0000-4000-8000-000000000001';
describe('closed workbench boundary', () => {
  it('preserves experience references and gaps and accepts older documents without them', () => {
    const legacy = Object.fromEntries(Object.entries(newDocument()).filter(([key]) => !['experienceSlots', 'experienceGaps'].includes(key)));
    expect(parseWorkDocument(legacy)).toMatchObject({ experienceSlots: [], experienceGaps: [] });
    const slot = { paragraphId: id, experienceRef: id, experienceRevision: 1 };
    expect(parseWorkDocument({ ...newDocument(), experienceSlots: [slot], experienceGaps: [id] })).toMatchObject({ experienceSlots: [slot], experienceGaps: [id] });
    expect(() => parseWorkDocument({ ...newDocument(), experienceSlots: [{ ...slot, accidentalText: 'synthetic' }] })).toThrow(WorkbenchContractError);
  });
  it('keeps queued writing without an invocation distinct from unknown and measured zero usage', () => {
    const queued={requestId:id,workRef:id,invocationRef:null,batchRef:id,taskConfigRef:id,state:'queued',inputRevision:id,action:'draft',
      methodVersion:'content-workbench-writing.v1',suggestion:null,sourceQualification:'readable',failureCode:null,staleBase:false,
      usage:{state:'not_reserved',inputTokens:null,outputTokens:null,chargedTokens:null,costUsd:null,measuredTokens:null,
        cachedInputTokens:null,cachedWriteTokens:null,reasoningTokens:null,costPicoUsd:null}};
    expect(parseAiStatus(queued)).toMatchObject({invocationRef:null,state:'queued',usage:{state:'not_reserved',chargedTokens:null}});
    const measured={...queued,invocationRef:id,state:'succeeded',usage:{...queued.usage,state:'measured',inputTokens:0,outputTokens:0,chargedTokens:0,measuredTokens:0,costPicoUsd:'0'}};
    expect(parseAiStatus(measured).usage.measuredTokens).toBe(0);
    expect(() => parseAiStatus({...queued,unexpected:true})).toThrow(WorkbenchContractError);
  });
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

describe('independent reading collection', () => {
  it('persists source identities without requiring a quote or canvas block', async () => {
    const { addMaterials, selectMaterial, parseMaterialCollection } = await import('./workbenchModel');
    const source = { documentId: `note:${id}`, sourceKind: 'note' as const, sourceRef: id, workRef: id, sourceVersion: 'a'.repeat(64), title: '作业启动', authorLabel: '作者未知', qualification: 'readable' as const, fragments: [], context: null, matchReasons: ['作业、启动'] };
    const selection = selectMaterial(source, 'prepared');
    const doc = parseWorkDocument({...newDocument(), materials: addMaterials([selection], [selection])});
    expect(doc.materials).toEqual([selection]);
    expect(doc.excerpts).toEqual([]);
    expect(doc.canvas).toEqual([]);
    const collection = { contract: 'content-workbench-materials.v1', asOf: 'now', methodVersion: 'workbench.lexical-context.v1', query: '作业启动', terms: ['作业','启动'], state: 'insufficient', gaps: ['评论不足'], documents: [source], nextCursor: null, searchScope: 'local_only' };
    expect(parseMaterialCollection(collection).gaps).toEqual(['评论不足']);
    expect(() => parseMaterialCollection({...collection, documents: [{...source, qualification:'everything_allowed'}]})).toThrow(WorkbenchContractError);
  });
  it('reads the bridge slot as undecided when a revision predates it, and rejects anything outside the closed shape', () => {
    const legacy = Object.fromEntries(Object.entries(newDocument()).filter(([key]) => !['bridge', 'coverAi'].includes(key)));
    expect(parseWorkDocument(legacy)).toMatchObject({ bridge: null, coverAi: null });
    // The server omits absent fields: only `skip` is always present.
    expect(parseWorkDocument({ ...newDocument(), bridge: { skip: true } }).bridge).toEqual({ skip: true, sentence: null, afterParagraph: null });
    const sentence = { sentenceCode: 'XJ-0001', sentenceRevision: 2, group: 'parent', text: '我也是学员家长' };
    const used = parseWorkDocument({ ...newDocument(), bridge: { skip: false, sentence, afterParagraph: id }, coverAi: true });
    expect(used.bridge).toEqual({ skip: false, sentence: { ...sentence, edited: false }, afterParagraph: id });
    expect(used.coverAi).toBe(true);
    expect(parseWorkDocument({ ...newDocument(), coverAi: false }).coverAi).toBe(false);
    for (const bad of [{ skip: 'yes' }, { skip: false, extra: 1 }, { skip: false, sentence: { ...sentence, group: 'nobody' } },
      { skip: false, sentence: { ...sentence, sentenceRevision: -1 } }, { skip: false, sentence: { ...sentence, unexpected: true } },
      { skip: false, afterParagraph: 'not-a-uuid' }]) {
      expect(() => parseWorkDocument({ ...newDocument(), bridge: bad })).toThrow(WorkbenchContractError);
    }
    expect(() => parseWorkDocument({ ...newDocument(), coverAi: 'maybe' })).toThrow(WorkbenchContractError);
  });
  it('reads an older document with an empty collection and rejects prose in identity metadata', () => {
    const legacy = Object.fromEntries(Object.entries(newDocument()).filter(([key]) => !['materials', 'materialPreparation'].includes(key)));
    expect(parseWorkDocument(legacy).materials).toEqual([]);
    expect(() => parseWorkDocument({...newDocument(), materials:[{documentId:`note:${id}`,sourceKind:'note',sourceRef:id,workRef:id,sourceVersion:'a'.repeat(64),addedBy:'manual',matchReasons:[],text:'不可在集合复制原文'}]})).toThrow(WorkbenchContractError);
  });
});
