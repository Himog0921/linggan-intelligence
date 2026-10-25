/** The only research reading contract. Raw is held in memory for explicit JSON export. */
export type MaterialKind = 'body' | 'tags' | 'image' | 'audio' | 'video_frame';
export type SourceState = 'present' | 'known_empty' | 'not_acquired' | 'not_applicable' | 'restricted';
export type Speaker = 'author' | 'other' | 'unconfirmed';
export type ReadingProducer = 'platform' | 'ordinary_ocr' | 'local_vision' | 'cloud_vision' | 'asr' | 'frame_ocr' | 'human_edit';
export type Adoption = 'automatic' | 'human_adopted' | 'human_edited';
export type Verification = 'unchecked' | 'verified';
export type ReadingReviewAction = 'adopt' | 'edit' | 'keep' | 'pending' | 'reject';
export interface ReadingReviewSummary {
  commandRef: string; actor: string; reason: string; action: ReadingReviewAction; reviewedAt: string;
}
export interface DossierIssue {
  key: string; code: string; message: string; resolution: 'reading_edit' | 'source_refresh'; blockRef?: string | null;
}
export interface BlockLocation {
  /** Normalized x, y, width, height. Never synthesized from reading order. */
  bbox?: readonly [number, number, number, number] | null;
  time?: { startMs: number; endMs: number } | null;
}
interface BlockBase extends BlockLocation {
  ref: string; speaker?: Speaker; issues?: DossierIssue[];
}
export type DossierBlock = BlockBase & (
  | { kind: 'heading'; text: string; level?: number }
  | { kind: 'paragraph' | 'quote' | 'gap'; text: string }
  | { kind: 'list'; items: string[]; ordered: boolean; start?: number }
  | { kind: 'table'; columns: string[]; rows: string[][] }
  | { kind: 'dialogue'; turns: { speaker: Speaker; text: string }[] }
);
export interface DossierMaterial {
  ref: string; kind: MaterialKind; slotKey: string | null; page: number | null;
  pageSource?: { basis: 'same_verified_bytes'; sourceRef: string; sourceVersion: string; slotKey: string };
  sourceState: SourceState; sourceRef: string | null; sourceVersion: string | null;
  currentRevisionRef: string | null; originalUrl: string | null;
  reading: { generatedBy: ReadingProducer; adoption: Adoption; verification: Verification; ruleVersion: string; review: ReadingReviewSummary | null } | null;
  processing?: { state: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled' | 'invalidated'; reason: string | null } | null;
  blocks: DossierBlock[]; issues: DossierIssue[];
}
export interface DossierNote {
  platform: string; sourceUrl: string | null; platformTitle: string | null;
  titleSource: { sourceRef: string | null; sourceVersion: string | null } | null;
  displayTitle: string | null; displayTitleBasis: 'platform' | 'cover' | 'missing'; titleBlockRefs: string[];
}
export interface DossierCoverage {
  imageInventory: { basis: 'platform_image_list' | 'observed_set'; expectedPages: number | null; acquiredPages: number; processedPages: number; missingPages: number[]; unknownOrder: boolean };
  sources: Record<'body' | 'images' | 'audio' | 'videoFrames', { state: 'available' | 'known_empty' | 'not_acquired' | 'not_applicable' | 'restricted' | 'partial' | 'not_processed'; units: number; readings: number }>;
  issues: DossierIssue[];
}
export interface DossierSelection {
  mode: 'full' | 'explicit'; kinds: MaterialKind[]; unitRefs: string[]; blockRefs: string[]; imagePages: number[];
  omittedUnits: number; completeRevision: string;
}
export interface Dossier {
  contract: 'research-dossier.v2'; workRef: string; revision: string; materialRevision: string; asOf: string;
  note: DossierNote; materials: DossierMaterial[]; readingView: { mainBlockRefs: string[]; supplementBlockRefs: string[] }; coverage: DossierCoverage;
  attachments: Record<'comments' | 'author' | 'interaction' | 'versions', { href: string }>;
  selection: DossierSelection; raw: unknown;
}
export interface RelayerPlan {
  workCount: number | null; imageSlots: number | null; imageJobs: number | null;
  layoutCount: number; pendingLayouts: number; alreadyCurrent: number; needsOcr: number; pendingReviewCount: number; expectedRevision: string;
}
export interface DossierPayload { dossier: Dossier; relayerPlan: RelayerPlan }
export interface ReadingCandidate {
  unitRef: string; slotKey: string | null; candidateRevisionRef: string; currentRevisionRef: string | null;
  blocks: DossierBlock[]; issues: DossierIssue[]; generatedBy: ReadingProducer; verification: Verification; createdAt: string; review: ReadingReviewSummary | null;
}
export interface ReadingReviewRequest {
  commandRef: string; expectedCurrentRevisionRef: string | null; candidateRevisionRef: string;
  action: ReadingReviewAction; editedBlocks?: DossierBlock[]; verified: boolean; resolvedIssueKeys: string[];
  actor: string; reason: string;
}
export interface ReadingReviewReceipt {
  commandRef: string; unitRef: string; candidateRevisionRef: string; action: ReadingReviewAction;
  currentRevisionRef: string | null; adoption: Adoption | null; verification: Verification | null; reviewRef: string | null;
}
export interface DossierComment { sourceRef: string; parentSourceRef: string | null; relation: 'ROOT' | 'REPLY'; body: string | null; likeCount: number | null }
export interface DossierCommentPage { items: DossierComment[]; total: number | null; nextCursor: string | null }
export interface RelayerReceipt { scopeCount: number; updated: number; alreadyCurrent: number; skipped: number; needsOcr: number }
