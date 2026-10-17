import { isRecord } from '../../../shared/api';
import { decode, list, num, obj, optBool, optNum, optObj, optStr, str, type Json } from '../../../shared/decode';
import type { Metric } from './evidenceTypes';
import { mediaPath } from './evidencePaths';
import type {
  CreatorPreview,
  Dossier,
  DossierComment,
  DossierCommentPage,
  DossierCoverage,
  DossierFragment,
  DossierLayering,
  DossierLine,
  DossierMedia,
  DossierMetadata,
  DossierPayload,
  DossierSection,
  RelayerPlan,
  RelayerReceipt,
  TaskInput,
  TaskKey,
} from './dossierTypes';

function metric(record: Json | null): Metric {
  return record === null ? { value: null, state: 'UNKNOWN' } : { value: optNum(record, 'value'), state: optStr(record, 'state') ?? 'UNKNOWN' };
}

function parseFragment(record: Json): DossierFragment {
  return {
    fragmentId: str(record, 'fragmentId'),
    field: optStr(record, 'field') ?? 'unknown',
    text: optStr(record, 'text') ?? '',
    end: optNum(record, 'end') ?? Array.from(optStr(record, 'text') ?? '').length,
    sourceRef: optStr(record, 'sourceRef'),
    sourceVersion: optStr(record, 'sourceVersion'),
  };
}

function parseSection(record: Json): DossierSection {
  const ids = Array.isArray(record.fragmentIds) ? record.fragmentIds.filter((id): id is string => typeof id === 'string') : [];
  return { label: str(record, 'label'), kind: optStr(record, 'kind') ?? '', fragmentIds: ids };
}

/** 归一化坐标须都在 0–1，否则不画框（宁可不标，也不标错位置）。 */
function parseBox(value: unknown): DossierLine['bbox'] {
  if (!Array.isArray(value) || value.length !== 4) return null;
  const [left, top, right, bottom] = value as unknown[];
  const box = [left, top, right, bottom];
  return box.every(side => typeof side === 'number' && Number.isFinite(side) && side >= 0 && side <= 1)
    ? ([left, top, right, bottom] as unknown as [number, number, number, number])
    : null;
}

function classifications(value: unknown): Record<string, string> {
  const result: Record<string, string> = {};
  if (Array.isArray(value)) {
    for (const entry of value) {
      if (isRecord(entry) && typeof entry.lineRef === 'string') result[entry.lineRef] = typeof entry.classification === 'string' ? entry.classification : 'uncertain';
    }
  }
  return result;
}

function parseLayering(record: Json | null): DossierLayering | null {
  if (record === null) return null;
  const lines = Array.isArray(record.lines) ? record.lines.filter(isRecord) : [];
  return {
    layerVersion: optStr(record, 'layerVersion'),
    state: optStr(record, 'state') ?? 'UNKNOWN',
    decisionSource: optStr(record, 'decisionSource'),
    engineVersion: optStr(record, 'engineVersion'),
    imageWidth: optNum(record, 'imageWidth'),
    imageHeight: optNum(record, 'imageHeight'),
    layoutRef: optStr(record, 'layoutRef'),
    layeringRef: optStr(record, 'layeringRef'),
    lines: lines.map(line => ({ lineRef: str(line, 'lineRef'), text: optStr(line, 'text') ?? '', bbox: parseBox(line.bbox) })),
    retained: classifications(record.retainedLines),
    excluded: classifications(record.excludedLines),
  };
}

function parseMedia(record: Json): DossierMedia {
  const other = optObj(record, 'otherProcessing');
  return {
    derivativeRef: optStr(record, 'derivativeRef'),
    slotKey: optStr(record, 'slotKey'),
    slotOrdinal: optNum(record, 'slotOrdinal'),
    state: optStr(record, 'state') ?? 'UNKNOWN',
    processorKind: optStr(record, 'processorKind'),
    contextOnly: optBool(record, 'contextOnly') === true,
    displayText: optStr(record, 'displayText'),
    researchText: optStr(record, 'researchText'),
    originalUrl: mediaPath(record.originalAssetUrl),
    originalMaterializationRef: optStr(record, 'originalMaterializationRef'),
    selectionBasis: optStr(record, 'selectionBasis'),
    layering: parseLayering(optObj(record, 'ocrLayering')),
    otherProcessing: other === null ? null : { state: optStr(other, 'state') ?? 'UNKNOWN' },
  };
}

function parseCoverage(record: Json): DossierCoverage {
  return {
    title: optStr(record, 'title') ?? 'UNKNOWN',
    body: optStr(record, 'body') ?? 'UNKNOWN',
    titleQualification: optStr(record, 'titleQualification'),
    mediaAvailable: optNum(record, 'mediaAvailable') ?? 0,
    mediaCount: optNum(record, 'mediaCount') ?? 0,
    uncertainImages: optNum(record, 'uncertainImages') ?? 0,
  };
}

function parseMetadata(record: Json): DossierMetadata {
  return {
    platform: optStr(record, 'platform') ?? '平台未知',
    authorDisplayName: optStr(record, 'authorDisplayName'),
    publishedAt: optStr(record, 'publishedAt'),
    likes: metric(optObj(record, 'likes')),
    comments: metric(optObj(record, 'comments')),
    collects: metric(optObj(record, 'collects')),
  };
}

export function parseRelayerPlan(record: Json): RelayerPlan {
  return {
    workCount: optNum(record, 'workCount'),
    imageSlots: optNum(record, 'imageSlots'),
    imageJobs: optNum(record, 'imageJobs'),
    layoutCount: num(record, 'layoutCount'),
    pendingLayouts: num(record, 'pendingLayouts'),
    alreadyCurrent: num(record, 'alreadyCurrent'),
    needsOcr: num(record, 'needsOcr'),
    maximumLayoutsPerWork: optNum(record, 'maximumLayoutsPerWork'),
    expectedRevision: str(record, 'expectedRevision'),
  };
}

function parseTaskInput(record: Json): TaskInput {
  const source = Array.isArray(record.fragments) ? record.fragments : Array.isArray(record.sources) ? record.sources : [];
  return {
    includedCharacterCount: optNum(record, 'includedCharacterCount') ?? 0,
    truncated: optBool(record, 'truncated') === true,
    fragments: source.filter(isRecord).map(fragment => {
      const text = optStr(fragment, 'text') ?? '';
      return {
        field: optStr(fragment, 'field') ?? optStr(fragment, 'kind') ?? 'unknown',
        characterCount: optNum(fragment, 'characterCount') ?? Array.from(text).length,
        text,
      };
    }),
    markdown: optStr(record, 'markdown'),
    raw: record,
  };
}

export function parseDossierPayload(value: unknown): DossierPayload {
  return decode(value, root => {
    const dossier = obj(root, 'dossier');
    const inputs = obj(root, 'inputs');
    const task = (key: TaskKey) => parseTaskInput(obj(inputs, key));
    return {
      dossier: {
        revision: str(dossier, 'revision'),
        asOf: optStr(dossier, 'asOf'),
        fragments: list(dossier, 'fragments', parseFragment),
        sections: list(dossier, 'sections', parseSection),
        media: list(dossier, 'media', parseMedia),
        coverage: parseCoverage(obj(dossier, 'coverage')),
        metadata: parseMetadata(obj(dossier, 'metadata')),
        raw: dossier,
      } satisfies Dossier,
      relayerPlan: parseRelayerPlan(obj(root, 'relayerPlan')),
      markdown: str(root, 'markdown'),
      inputs: { creator: task('creator'), comments: task('comments'), topics: task('topics') },
    };
  });
}

export function parseRelayerPlanResponse(value: unknown): RelayerPlan {
  return decode(value, root => parseRelayerPlan(obj(root, 'relayerPlan')));
}

export function parseCreatorPreview(value: unknown): CreatorPreview {
  return decode(value, root => {
    const input = optObj(root, 'input');
    return {
      available: optBool(root, 'available') === true,
      reason: optStr(root, 'reason'),
      inputTokenLimit: optNum(root, 'inputTokenLimit'),
      estimatedInputTokensUpperBound: optNum(root, 'estimatedInputTokensUpperBound'),
      materials: input === null ? null : optStr(input, 'materials'),
      raw: root,
    };
  });
}

function parseDossierComment(record: Json): DossierComment {
  return {
    sourceRef: str(record, 'sourceRef'),
    parentSourceRef: optStr(record, 'parentSourceRef'),
    relation: optStr(record, 'relation') === 'REPLY' ? 'REPLY' : 'ROOT',
    body: optStr(record, 'body'),
    likeCount: optNum(record, 'likeCount'),
  };
}

export function parseDossierComments(value: unknown): DossierCommentPage {
  return decode(value, root => ({
    items: list(root, 'items', parseDossierComment),
    total: optNum(root, 'total'),
    nextCursor: optStr(root, 'nextCursor'),
  }));
}

export function parseRelayerReceipt(value: unknown): RelayerReceipt {
  return decode(value, root => ({
    scopeCount: num(root, 'scopeCount'),
    updated: num(root, 'updated'),
    alreadyCurrent: num(root, 'alreadyCurrent'),
    skipped: num(root, 'skipped'),
    needsOcr: optNum(root, 'needsOcr') ?? 0,
  }));
}
