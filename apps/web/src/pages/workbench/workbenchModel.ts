import { parseBlock, parseIssue } from '../corpus/evidence/dossierBlockDecode';
import type { DossierBlock, DossierIssue, Speaker } from '../corpus/evidence/dossierTypes';
/** Closed HTTP projection of CONTENT-WORKBENCH-001. Unicode ranges are scalar offsets. */
export type SourceKind = 'note' | 'comment' | 'ocr' | 'transcript' | 'frame_text' | 'ops_publication';
export type SourceField = 'title' | 'body' | 'tags' | 'ocr' | 'transcript' | 'frame_text' | 'comment_body';
export type CanvasGroup = 'inbox' | 'opening' | 'argument' | 'example' | 'ending';
export type PageKind = 'cover' | 'opinion' | 'voice' | 'steps';
export type Palette = 'cream' | 'sage' | 'ink';
export interface OcrLocation {
  unitRef: string;
  readingRef: string;
  blockRef: string;
  slotKey: string;
  bbox: [number, number, number, number];
}
export interface Citation {
  asOf: string;
  blockRef: string;
  sourceKind: SourceKind;
  sourceRef: string;
  workRef: string | null;
  field: SourceField;
  sourceVersion: string;
  start: number;
  end: number;
  textHash: string;
  location: OcrLocation | null;
}
export interface Paragraph {
  id: string;
  text: string;
}
export interface Excerpt {
  id: string;
  citation: Citation;
  selectedText: string | null;
  textHash: string;
}
export interface CanvasBlock {
  id: string;
  excerptId: string | null;
  group: CanvasGroup;
  note: string;
  expression: string;
  expressionRevision: number;
}
export interface PageBlock {
  id: string;
  text: string;
}
export interface Crop {
  x: number;
  y: number;
  width: number;
  height: number;
}
export interface PageStyle {
  palette: Palette;
  fontFamily: string;
  width: number;
  height: number;
  alignment: 'start' | 'center' | 'end';
}
export interface ImagePage {
  id: string;
  kind: PageKind;
  templateVersion: string;
  title: string;
  subtitle: string;
  blocks: PageBlock[];
  assetId: string | null;
  crop: Crop;
  style: PageStyle;
}
export type UseTarget = {
  kind: 'paragraph';
  paragraphId: string;
} | {
  kind: 'pageBlock';
  pageId: string;
  blockId: string;
} | {
  kind: 'title' | 'angle';
} | {
  kind: 'outline' | 'tag';
  index: number;
} | {
  kind: 'canvasNote' | 'canvasExpression';
  canvasId: string;
} | {
  kind: 'pageTitle' | 'pageSubtitle';
  pageId: string;
};
export interface MaterialUse {
  id: string;
  canvasId: string;
  expressionRevision: number;
  excerptId: string | null;
  target: UseTarget;
  placedTextHash: string;
  state: 'current' | 'modified' | 'needs_review';
  contentOrigin: 'excerpt' | 'expression';
  sourceSpan: {
    start: number;
    end: number;
    textHash: string;
  } | null;
}
export type MaterialKind = 'note' | 'comment' | 'ops_publication';
export interface MaterialSelection {
  documentId: string;
  sourceKind: MaterialKind;
  sourceRef: string;
  workRef: string | null;
  sourceVersion: string;
  addedBy: 'prepared' | 'manual';
  matchReasons: string[];
}
export interface MaterialPreparation {
  methodVersion: string;
  query: string;
  terms: string[];
  state: 'ready' | 'insufficient';
  gaps: string[];
}
export interface MaterialDocument {
  documentId: string;
  sourceKind: MaterialKind;
  sourceRef: string;
  workRef: string | null;
  sourceVersion: string;
  title: string;
  authorLabel: string;
  qualification: 'readable' | 'changed' | 'restricted' | 'unavailable';
  fragments: SourceItem[];
  context: unknown;
  matchReasons: string[];
}
export interface MaterialCollection extends MaterialPreparation {
  contract: 'content-workbench-materials.v1';
  asOf: string;
  documents: MaterialDocument[];
  nextCursor: number | null;
  searchScope: string;
}
/** Whose voice a bridge sentence is written in (CREATION-ASSETS-001). */
export type BridgeGroup = 'parent' | 'coach';
/** The library sentence a draft uses; `text` is what this draft says and may be rewritten locally. */
export interface BridgeUse {
  sentenceCode: string;
  sentenceRevision: number;
  group: BridgeGroup;
  text: string;
  edited: boolean;
}
/** The bridge slot: `skip` is "这篇不植入"; no sentence and no skip means still undecided. */
export interface BridgeChoice {
  skip: boolean;
  sentence: BridgeUse | null;
  afterParagraph: string | null;
}
/** Each paragraph derived from an experience retains its originating revision. */
export interface ExperienceSlot {
  paragraphId: string;
  experienceRef: string;
  experienceRevision: number;
}
export interface WorkDocument {
  contract: string;
  experienceEpoch: string | null;
  experienceSlots: ExperienceSlot[];
  experienceGaps: string[];
  /** Absent in older revisions; null means the bridge slot has not been decided. */
  bridge: BridgeChoice | null;
  /** Whether the cover image is AI-made; null means not recorded, never "no". */
  coverAi: boolean | null;
  materials: MaterialSelection[];
  materialPreparation: MaterialPreparation | null;
  title: string;
  accountRef: string | null;
  topicRef: string | null;
  ideaRef: string | null;
  ideaName: string | null;
  angle: string;
  outline: string[];
  tags: string[];
  coverTitleCandidates: string[];
  noteTitleCandidates: string[];
  paragraphs: Paragraph[];
  excerpts: Excerpt[];
  canvas: CanvasBlock[];
  pages: ImagePage[];
  uses: MaterialUse[];
  derivedSources: { target: UseTarget; citation: Citation }[];
}
export interface Revision {
  id: string;
  number: number;
  savedAt: string;
  document: WorkDocument;
}
export interface RevisionSummary {
  id: string;
  number: number;
  savedAt: string;
  referenceFormat: 'current' | 'legacy_reference' | 'unsupported';
}
export interface SaveReceipt {
  requestId: string;
  workId: string;
  outcome: string;
  replayed: boolean;
  revision: Revision;
}
export interface SourceStatus {
  excerptId: string;
  state: string;
  code: string;
}
export interface ExportPage {
  pageId: string;
  ordinal: number;
  sha256: string | null;
  byteSize: number | null;
}
export interface ExportRecord {
  id: string;
  createdAt?: string | null;
  workId: string;
  revisionId: string;
  state: string;
  failureCode: string | null;
  rendererVersion: string;
  fontVersion: string;
  pages: ExportPage[];
  packageSha256: string | null;
  packageByteSize: number | null;
}
export interface WorkEnvelope {
  experienceResetRequired?: boolean;
  workId: string;
  domainRef: string;
  savedBy: string;
  revision: Revision;
  sourceStatuses: SourceStatus[];
  exports: ExportRecord[];
}
export interface WorkSummary {
  workId: string;
  domainRef: string;
  title: string;
  revisionId: string;
  savedAt: string;
  copiedRevisionId?: string | null;
  publishedRevisionId?: string | null;
  publicationUnavailable?: boolean;
  ideaRef?: string | null;
}
export interface IdeaBoxSummary { ideaRef: string; anchorWorkId: string; anchorRevisionId: string; savedAt: string; name: string; intent: string }
export interface AssetRecord {
  id: string;
  workId: string;
  sha256: string;
  byteSize: number;
  mimeType: string;
  width: number;
  height: number;
}
export interface SourceItem {
  asOf: string;
  fragmentId: string;
  blockRef: string;
  speaker: Speaker;
  structure: DossierBlock | null;
  issues: DossierIssue[];
  sourceKind: SourceKind;
  sourceRef: string;
  workRef: string | null;
  field: SourceField;
  sourceVersion: string;
  title: string;
  text: string;
  bodyState: string;
  authorLabel: string;
  qualification: string;
  location: OcrLocation | null;
}
export interface SourceList {
  contract: string;
  asOf: string;
  items: SourceItem[];
  distribution: {
    sourceKind: SourceKind;
    count: number;
    basis: string;
  }[];
  total: number;
  totalBasis: string;
  returnedSourceCount: number;
  returnedFragmentCount: number;
  nextCursor: number | null;
  searchScope: string;
  scope: string;
}
export interface ValidatedAnchor {
  citation: Citation;
  selectedText: string;
  textHash: string;
}
export interface SourceContext {
  item: SourceItem;
  context: unknown;
}
export type WritingAction = 'draft' | 'rewrite_selection' | 'page_copy';
export interface WritingSelection {
  paragraphId: string;
  start: number;
  end: number;
  textHash: string;
}
export interface AiReadiness {
  configured: boolean;
  transportCallable: boolean | null;
  writingQualified: boolean | null;
  configRef: string | null;
  methodVersion: string;
  modelRef?: string | null;
  connectionVersionRef?: string;
  qualificationScope?: string;
  taskConfigRef?: string;
  deniedReasons?: string[];
}
export interface AiPreview {
  previewRef: string;
  inputRevision: string;
  inputHash: string;
  action: WritingAction;
  methodVersion: string;
  configRef: string;
  modelRef: string | null;
  connectionVersionRef: string;
  sourceCount: number;
  inputCharacterCount: number;
  estimatedInputTokens: number;
  reservedTokens: number;
  maxCalls: number;
  outboundInput: unknown;
  outboundSystem?: string;
  confirmationRequired: boolean;
  taskConfigRef?: string;
}
export interface WritingSuggestion {
  title: string | null;
  paragraphs: {
    paragraphId: string | null;
    text: string;
  }[];
  pages: {
    pageId: string | null;
    heading: string;
    body: string;
  }[];
}
export interface AiStatus {
  requestId: string;
  workRef: string;
  invocationRef: string | null;
  state: string;
  inputRevision: string;
  action: WritingAction;
  methodVersion: string;
  suggestion: WritingSuggestion | null;
  sourceQualification: string;
  failureCode: string | null;
  staleBase: boolean;
  batchRef?: string | null;
  taskConfigRef?: string | null;
  usage: {
    state: string;
    inputTokens: number | null;
    outputTokens: number | null;
    chargedTokens: number | null;
    costUsd: number | null;
    measuredTokens?: number | null;
    cachedInputTokens?: number | null;
    cachedWriteTokens?: number | null;
    reasoningTokens?: number | null;
    costPicoUsd?: string | null;
  };
}
export type TitleStatus = Omit<AiStatus, 'action' | 'suggestion'> & {
  action: 'title_ideas';
  suggestion: { coverTitles: string[]; noteTitles: string[] } | null;
  titleWarnings: { cover: boolean[]; note: boolean[] } | null;
};
type Parser<T> = (value: unknown) => T;
export class WorkbenchContractError extends Error {
  constructor() {
    super('创作接口返回不符合合同，未采用返回内容。');
    this.name = 'WorkbenchContractError';
  }
}
function bad(): never {
  throw new WorkbenchContractError();
}
export function object(value: unknown): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return bad();
  return value as Record<string, unknown>;
}
const str: Parser<string> = v => typeof v === 'string' ? v : bad();
const num: Parser<number> = v => typeof v === 'number' && Number.isFinite(v) ? v : bad();
const integer: Parser<number> = v => {
  const n = num(v);
  return Number.isSafeInteger(n) && n >= 0 ? n : bad();
};
const bool: Parser<boolean> = v => typeof v === 'boolean' ? v : bad();
export const uuid: Parser<string> = v => {
  const s = str(v);
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(s) ? s : bad();
};
const nullable = <T,>(p: Parser<T>): Parser<T | null> => v => v === null ? null : p(v);
const array = <T,>(p: Parser<T>): Parser<T[]> => v => Array.isArray(v) ? v.map(p) : bad();
const choice = <T extends string,>(...values: T[]): Parser<T> => v => {
  for (const x of values) if (v === x) return x;
  return bad();
};
function shape<T>(fields: { [K in keyof T]: Parser<T[K]> }): Parser<T> {
  return value => {
    const v = object(value);
    const keys = Object.keys(fields);
    if (Object.keys(v).some(k => !keys.includes(k))) return bad();
    const result: Record<string, unknown> = {};
    for (const k of keys) {
      const parser = Reflect.get(fields, k) as Parser<unknown>;
      result[k] = parser(v[k]);
    }
    return result as T;
  };
}
const sourceKind = choice('note', 'comment', 'ocr', 'transcript', 'frame_text', 'ops_publication');
const field = choice('title', 'body', 'tags', 'ocr', 'transcript', 'frame_text', 'comment_body');
const bbox: Parser<[number, number, number, number]> = v => {
  if (!Array.isArray(v) || v.length !== 4) return bad();
  const result: [number, number, number, number] = [num(v[0]), num(v[1]), num(v[2]), num(v[3])];
  if (result.some(value => value < 0 || value > 1) || result[2] <= 0 || result[3] <= 0 || result[0] + result[2] > 1.00000001 || result[1] + result[3] > 1.00000001) return bad();
  return result;
};
const location = shape<OcrLocation>({ unitRef: uuid, readingRef: uuid, blockRef: str, slotKey: str, bbox });
export const parseCitation = shape<Citation>({
  asOf: str,
  blockRef: str,
  sourceKind,
  sourceRef: uuid,
  workRef: nullable(uuid),
  field,
  sourceVersion: str,
  start: integer,
  end: integer,
  textHash: str,
  location: nullable(location)
});
const paragraph = shape<Paragraph>({
  id: uuid,
  text: str
});
const excerpt = shape<Excerpt>({
  id: uuid,
  citation: parseCitation,
  selectedText: nullable(str),
  textHash: str
});
const canvas = shape<CanvasBlock>({
  id: uuid,
  excerptId: nullable(uuid),
  group: choice('inbox', 'opening', 'argument', 'example', 'ending'),
  note: str,
  expression: str,
  expressionRevision: integer
});
const pageBlock = shape<PageBlock>({
  id: uuid,
  text: str
});
export const parseImagePage = shape<ImagePage>({
  id: uuid,
  kind: choice('cover', 'opinion', 'voice', 'steps'),
  templateVersion: str,
  title: str,
  subtitle: str,
  blocks: array(pageBlock),
  assetId: nullable(uuid),
  crop: shape<Crop>({
    x: num,
    y: num,
    width: num,
    height: num
  }),
  style: shape<PageStyle>({
    palette: choice('cream', 'sage', 'ink'),
    fontFamily: str,
    width: integer,
    height: integer,
    alignment: choice('start', 'center', 'end')
  })
});
export const parseUseTarget: Parser<UseTarget> = v => {
  switch (object(v).kind) {
    case 'paragraph':
      return shape<{
        kind: 'paragraph';
        paragraphId: string;
      }>({
        kind: choice('paragraph'),
        paragraphId: uuid
      })(v);
    case 'pageBlock':
      return shape<{
        kind: 'pageBlock';
        pageId: string;
        blockId: string;
      }>({
        kind: choice('pageBlock'),
        pageId: uuid,
        blockId: uuid
      })(v);
    case 'title':
    case 'angle':
      return shape<{
        kind: 'title' | 'angle';
      }>({
        kind: choice('title', 'angle')
      })(v);
    case 'outline':
    case 'tag':
      return shape<{
        kind: 'outline' | 'tag';
        index: number;
      }>({
        kind: choice('outline', 'tag'),
        index: integer
      })(v);
    case 'canvasNote':
    case 'canvasExpression':
      return shape<{
        kind: 'canvasNote' | 'canvasExpression';
        canvasId: string;
      }>({
        kind: choice('canvasNote', 'canvasExpression'),
        canvasId: uuid
      })(v);
    case 'pageTitle':
    case 'pageSubtitle':
      return shape<{
        kind: 'pageTitle' | 'pageSubtitle';
        pageId: string;
      }>({
        kind: choice('pageTitle', 'pageSubtitle'),
        pageId: uuid
      })(v);
    default:
      return bad();
  }
};
const materialUse = shape<MaterialUse>({
  id: uuid,
  canvasId: uuid,
  expressionRevision: integer,
  excerptId: nullable(uuid),
  target: parseUseTarget,
  placedTextHash: str,
  state: choice('current', 'modified', 'needs_review'),
  contentOrigin: choice('excerpt', 'expression'),
  sourceSpan: nullable(shape({
    start: integer,
    end: integer,
    textHash: str
  }))
});
const materialKind = choice('note', 'comment', 'ops_publication');
export const parseMaterialSelection = shape<MaterialSelection>({
  documentId: str, sourceKind: materialKind, sourceRef: uuid, workRef: nullable(uuid),
  sourceVersion: str, addedBy: choice('prepared', 'manual'), matchReasons: array(str)
});
const parseMaterialPreparation = shape<MaterialPreparation>({
  methodVersion: str, query: str, terms: array(str), state: choice('ready', 'insufficient'), gaps: array(str)
});
const parseBridgeUse = shape<BridgeUse>({
  sentenceCode: str, sentenceRevision: integer, group: choice('parent', 'coach'),
  text: str, edited: v => v === undefined ? false : bool(v)
});
export const parseBridgeChoice = shape<BridgeChoice>({
  skip: v => v === undefined ? false : bool(v),
  sentence: v => v === undefined ? null : nullable(parseBridgeUse)(v),
  afterParagraph: v => v === undefined ? null : nullable(uuid)(v)
});
export const parseWorkDocument = shape<WorkDocument>({
  contract: v => v === 'content-workbench.v1' ? v : bad(),
  experienceEpoch: v => v === undefined ? null : nullable(uuid)(v),
  experienceSlots: v => v === undefined ? [] : array(shape<ExperienceSlot>({ paragraphId: uuid, experienceRef: uuid, experienceRevision: integer }))(v),
  experienceGaps: v => v === undefined ? [] : array(uuid)(v),
  bridge: v => v === undefined ? null : nullable(parseBridgeChoice)(v),
  coverAi: v => v === undefined ? null : nullable(bool)(v),
  materials: v => v === undefined ? [] : array(parseMaterialSelection)(v),
  materialPreparation: v => v === undefined ? null : nullable(parseMaterialPreparation)(v),
  title: str,
  accountRef: nullable(uuid),
  topicRef: nullable(uuid),
  ideaRef: v => v === undefined ? null : nullable(uuid)(v),
  ideaName: v => v === undefined ? null : nullable(str)(v),
  angle: str,
  outline: array(str),
  tags: array(str),
  coverTitleCandidates: v => v === undefined ? [] : array(str)(v),
  noteTitleCandidates: v => v === undefined ? [] : array(str)(v),
  paragraphs: array(paragraph),
  excerpts: array(excerpt),
  canvas: array(canvas),
  pages: array(parseImagePage),
  uses: array(materialUse),
  derivedSources: v => v === undefined ? [] : array(shape({ target: parseUseTarget, citation: parseCitation }))(v)
});
export const parseRevision = shape<Revision>({
  id: uuid,
  number: integer,
  savedAt: str,
  document: parseWorkDocument
});
export const parseSaveReceipt = shape<SaveReceipt>({
  requestId: uuid,
  workId: uuid,
  outcome: str,
  replayed: bool,
  revision: parseRevision
});
export const parseExportRecord = shape<ExportRecord>({
  id: uuid,
  createdAt: value => value === undefined ? undefined : nullable(str)(value),
  workId: uuid,
  revisionId: uuid,
  state: str,
  failureCode: nullable(str),
  rendererVersion: str,
  fontVersion: str,
  pages: array(shape<ExportPage>({
    pageId: uuid,
    ordinal: integer,
    sha256: nullable(str),
    byteSize: nullable(integer)
  })),
  packageSha256: nullable(str),
  packageByteSize: nullable(integer)
});
export const parseEnvelope = shape<WorkEnvelope>({
  experienceResetRequired: v => v === undefined ? false : bool(v),
  workId: uuid,
  domainRef: uuid,
  savedBy: str,
  revision: parseRevision,
  sourceStatuses: array(shape<SourceStatus>({
    excerptId: uuid,
    state: str,
    code: str
  })),
  exports: array(parseExportRecord)
});
export const parseWorkList = shape<{
  items: WorkSummary[];
}>({
  items: array(shape<WorkSummary>({
    workId: uuid,
    domainRef: uuid,
    title: str,
    revisionId: uuid,
    savedAt: str,
    copiedRevisionId: value => value === undefined ? undefined : nullable(uuid)(value),
    publishedRevisionId: value => value === undefined ? undefined : nullable(uuid)(value),
    publicationUnavailable: value => value === undefined ? undefined : bool(value),
    ideaRef: value => value === undefined ? undefined : nullable(uuid)(value)
  }))
});
export const parseIdeaBoxList = shape<{ items: IdeaBoxSummary[] }>({ items: array(shape<IdeaBoxSummary>({
  ideaRef: uuid, anchorWorkId: uuid, anchorRevisionId: uuid, savedAt: str, name: str, intent: str,
})) });
export const parseHistory = shape<{
  items: RevisionSummary[];
}>({
  items: array(shape<RevisionSummary>({
    id: uuid, number: integer, savedAt: str,
    referenceFormat: value => {
      if (value !== 'current' && value !== 'legacy_reference' && value !== 'unsupported') throw new Error('Invalid revision format');
      return value;
    },
  }))
});
export const parseAsset = shape<AssetRecord>({
  id: uuid,
  workId: uuid,
  sha256: str,
  byteSize: integer,
  mimeType: str,
  width: integer,
  height: integer
});
const sourceItem = shape<SourceItem>({
  asOf: str,
  fragmentId: str,
  blockRef: str,
  speaker: choice('author', 'other', 'unconfirmed'),
  structure: nullable(v => parseBlock(object(v))),
  issues: array(v => parseIssue(object(v))),
  sourceKind,
  sourceRef: uuid,
  workRef: nullable(uuid),
  field,
  sourceVersion: str,
  title: str,
  text: str,
  bodyState: str,
  authorLabel: str,
  qualification: str,
  location: nullable(location)
});
export function parseSourceItem(value: unknown): SourceItem {
  const row = sourceItem(value);
  if (!row.blockRef || !row.asOf || row.fragmentId !== row.blockRef || (row.structure !== null && row.structure.ref !== row.blockRef)) return bad();
  return row;
}
export const parseMaterialDocument = shape<MaterialDocument>({
  documentId: str, sourceKind: materialKind, sourceRef: uuid, workRef: nullable(uuid),
  sourceVersion: str, title: str, authorLabel: str,
  qualification: choice('readable', 'changed', 'restricted', 'unavailable'),
  fragments: array(parseSourceItem), context: v => v, matchReasons: array(str)
});
export const parseMaterialCollection = shape<MaterialCollection>({
  contract: v => v === 'content-workbench-materials.v1' ? v : bad(),
  asOf: str, methodVersion: str, query: str, terms: array(str), state: choice('ready', 'insufficient'),
  gaps: array(str), documents: array(parseMaterialDocument), nextCursor: nullable(integer), searchScope: str
});
export function selectMaterial(document: MaterialDocument, addedBy: MaterialSelection['addedBy']): MaterialSelection {
  return { documentId: document.documentId, sourceKind: document.sourceKind, sourceRef: document.sourceRef,
    workRef: document.workRef, sourceVersion: document.sourceVersion, addedBy, matchReasons: document.matchReasons };
}
export function addMaterials(current: MaterialSelection[], incoming: MaterialSelection[]): MaterialSelection[] {
  const seen = new Set(current.map(m => m.documentId));
  return [...current, ...incoming.filter(m => { if (seen.has(m.documentId)) return false; seen.add(m.documentId); return true; })];
}
export const parseSourceList = shape<SourceList>({
  contract: v => v === 'content-workbench-sources.v1' ? v : bad(),
  asOf: str,
  items: array(parseSourceItem),
  distribution: array(shape({
    sourceKind,
    count: integer,
    basis: str
  })),
  total: integer,
  totalBasis: str,
  returnedSourceCount: integer,
  returnedFragmentCount: integer,
  nextCursor: nullable(integer),
  searchScope: str,
  scope: str
});
export const parseAnchor = shape<ValidatedAnchor>({
  citation: parseCitation,
  selectedText: str,
  textHash: str
});
export const parseContext = shape<SourceContext>({
  item: parseSourceItem,
  context: v => v
});
export const parseFrozen = shape<{
  export: ExportRecord;
  revision: Revision;
}>({
  export: parseExportRecord,
  revision: parseRevision
});
const optional = <T,>(p: Parser<T>): Parser<T | undefined> => v => v === undefined ? undefined : p(v);
export const parseReadiness = shape<AiReadiness>({
  configured: bool,
  transportCallable: nullable(bool),
  writingQualified: nullable(bool),
  configRef: nullable(uuid),
  methodVersion: str,
  modelRef: optional(nullable(uuid)),
  connectionVersionRef: optional(uuid),
  qualificationScope: optional(str),
  taskConfigRef: optional(uuid),
  deniedReasons: optional(array(str))
});
const action = choice('draft', 'rewrite_selection', 'page_copy');
export const parsePreview = shape<AiPreview>({
  previewRef: uuid,
  inputRevision: uuid,
  inputHash: str,
  action,
  methodVersion: str,
  configRef: uuid,
  modelRef: nullable(uuid),
  connectionVersionRef: uuid,
  sourceCount: integer,
  inputCharacterCount: integer,
  estimatedInputTokens: integer,
  reservedTokens: integer,
  maxCalls: integer,
  outboundInput: v => v,
  outboundSystem: optional(str),
  confirmationRequired: bool,
  taskConfigRef: optional(uuid)
});
export const parseWritingSuggestion = shape<WritingSuggestion>({
  title: nullable(str),
  paragraphs: array(shape({
    paragraphId: nullable(uuid),
    text: str
  })),
  pages: array(shape({
    pageId: nullable(uuid),
    heading: str,
    body: str
  }))
});
export const parseAiStatus = shape<AiStatus>({
  requestId: uuid,
  workRef: uuid,
  invocationRef: nullable(uuid),
  state: str,
  inputRevision: uuid,
  action,
  methodVersion: str,
  suggestion: nullable(parseWritingSuggestion),
  sourceQualification: str,
  failureCode: nullable(str),
  staleBase: bool,
  batchRef: optional(nullable(uuid)),
  taskConfigRef: optional(nullable(uuid)),
  usage: shape({
    state: str,
    inputTokens: nullable(integer),
    outputTokens: nullable(integer),
    chargedTokens: nullable(integer),
    costUsd: nullable(num),
    measuredTokens: optional(nullable(integer)),
    cachedInputTokens: optional(nullable(integer)),
    cachedWriteTokens: optional(nullable(integer)),
    reasoningTokens: optional(nullable(integer)),
    costPicoUsd: optional(nullable(str))
  })
});
export function parseTitleStatus(value: unknown): TitleStatus {
  const raw = object(value);
  if (raw.action !== 'title_ideas') return bad();
  const { titleWarnings, ...baseFields } = raw;
  const base = parseAiStatus({ ...baseFields, action: 'draft', suggestion: null });
  const suggestion = raw.suggestion === null ? null : shape<{coverTitles:string[]; noteTitles:string[]}>({coverTitles:array(str), noteTitles:array(str)})(raw.suggestion);
  if (suggestion && (suggestion.coverTitles.length === 0 || suggestion.coverTitles.length > 10 || suggestion.noteTitles.length === 0 || suggestion.noteTitles.length > 10 || suggestion.coverTitles.some(title => !title.trim() || Array.from(title).length > 10) || suggestion.noteTitles.some(title => !title.trim() || Array.from(title).length > 20))) return bad();
  const warnings = titleWarnings === null ? null : shape<{cover:boolean[];note:boolean[]}>({cover:array(bool),note:array(bool)})(titleWarnings);
  if ((suggestion === null) !== (warnings === null) || (suggestion && warnings && (warnings.cover.length !== suggestion.coverTitles.length || warnings.note.length !== suggestion.noteTitles.length))) return bad();
  return { ...base, action: 'title_ideas', suggestion, titleWarnings: warnings };
}
export function newDocument(title = ''): WorkDocument {
  return {
    contract: 'content-workbench.v1',
    experienceEpoch: null,
    experienceSlots: [],
    experienceGaps: [],
    bridge: null,
    coverAi: null,
    materials: [],
    materialPreparation: null,
    title,
    accountRef: null,
    topicRef: null,
    ideaRef: null,
    ideaName: null,
    angle: '',
    outline: [],
    tags: [],
    coverTitleCandidates: [],
    noteTitleCandidates: [],
    paragraphs: [],
    excerpts: [],
    canvas: [],
    pages: [],
    uses: [],
    derivedSources: []
  };
}
export function scalarRange(text: string, start: number, end: number): {
  start: number;
  end: number;
  text: string;
} {
  return {
    start: Array.from(text.slice(0, start)).length,
    end: Array.from(text.slice(0, end)).length,
    text: text.slice(start, end)
  };
}
export const GROUPS: readonly {
  id: CanvasGroup;
  label: string;
}[] = [{
  id: 'inbox',
  label: '待整理'
}, {
  id: 'opening',
  label: '开头'
}, {
  id: 'argument',
  label: '论点'
}, {
  id: 'example',
  label: '例子'
}, {
  id: 'ending',
  label: '结尾'
}];
export const SOURCE_LABELS: Record<SourceKind, string> = {
  note: '原笔记',
  comment: '匿名原声',
  ocr: '图片文字',
  transcript: '音轨转写',
  frame_text: '视频画面文字',
  ops_publication: '我方作品'
};

/** New authorship starts at revision one, matching Rust canvas validation. */
export function newCanvasBlock(group: CanvasGroup = 'inbox', excerptId: string | null = null): CanvasBlock {
  return {
    id: crypto.randomUUID(),
    excerptId,
    group,
    note: '',
    expression: '',
    expressionRevision: 1
  };
}
