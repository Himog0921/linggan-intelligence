/**
 * 研究底稿（RESEARCH-DOSSIER-001）的接口响应形状。
 *
 * `raw` 字段只服务两件事：用户显式触发的导出（JSON 导出就是服务器给的底稿原样序列化，旧页如此，DR-9），
 * 以及折叠的技术诊断。普通层只渲染白名单字段，不渲染 `raw`。
 */
import type { Metric } from './evidenceTypes';

export interface DossierFragment {
  fragmentId: string;
  field: string;
  text: string;
  /** 片段在底稿里的字符数（旧页用 `end` 当长度，起点恒为 0）。 */
  end: number;
  sourceRef: string | null;
  sourceVersion: string | null;
}

export interface DossierSection {
  label: string;
  kind: string;
  fragmentIds: string[];
}

export interface DossierLine {
  lineRef: string;
  text: string;
  /** 归一化坐标 [左, 上, 右, 下]，须都在 0–1；否则为 null，不画框。 */
  bbox: readonly [number, number, number, number] | null;
}

export interface DossierLayering {
  layerVersion: string | null;
  state: string;
  decisionSource: string | null;
  engineVersion: string | null;
  imageWidth: number | null;
  imageHeight: number | null;
  layoutRef: string | null;
  layeringRef: string | null;
  lines: DossierLine[];
  /** 保留行：行引用 → 分类。 */
  retained: Readonly<Record<string, string>>;
  /** 已排除行（水印、界面、不清文字）：行引用 → 分类。 */
  excluded: Readonly<Record<string, string>>;
}

export interface DossierMedia {
  derivativeRef: string | null;
  slotKey: string | null;
  slotOrdinal: number | null;
  state: string;
  processorKind: string | null;
  contextOnly: boolean;
  displayText: string | null;
  researchText: string | null;
  /** 原图的受控本地句柄（已过 `/api/local/media/` 校验）；没有是 null。 */
  originalUrl: string | null;
  originalMaterializationRef: string | null;
  selectionBasis: string | null;
  layering: DossierLayering | null;
  /** 同一张图另有处理结果时的提示（当前展示所选可读版本）。 */
  otherProcessing: { state: string } | null;
}

export interface DossierCoverage {
  title: string;
  body: string;
  titleQualification: string | null;
  mediaAvailable: number;
  mediaCount: number;
  uncertainImages: number;
}

export interface DossierMetadata {
  platform: string;
  authorDisplayName: string | null;
  publishedAt: string | null;
  likes: Metric;
  comments: Metric;
  collects: Metric;
}

export interface RelayerPlan {
  workCount: number | null;
  imageSlots: number | null;
  imageJobs: number | null;
  layoutCount: number;
  pendingLayouts: number;
  alreadyCurrent: number;
  needsOcr: number;
  maximumLayoutsPerWork: number | null;
  expectedRevision: string;
}

export interface TaskInput {
  includedCharacterCount: number;
  truncated: boolean;
  fragments: { field: string; characterCount: number; text: string }[];
  markdown: string | null;
  raw: unknown;
}

export type TaskKey = 'creator' | 'comments' | 'topics';

export interface Dossier {
  revision: string;
  asOf: string | null;
  fragments: DossierFragment[];
  sections: DossierSection[];
  media: DossierMedia[];
  coverage: DossierCoverage;
  metadata: DossierMetadata;
  /** 服务器给的底稿原样（含 `metadata.authorExternalId`）：只用于显式导出与折叠诊断。 */
  raw: unknown;
}

export interface DossierPayload {
  dossier: Dossier;
  relayerPlan: RelayerPlan;
  markdown: string;
  inputs: Record<TaskKey, TaskInput>;
}

export interface CreatorPreview {
  available: boolean;
  reason: string | null;
  inputTokenLimit: number | null;
  estimatedInputTokensUpperBound: number | null;
  materials: string | null;
  /** 完整请求与来源映射（含系统提示词）：只作折叠诊断，不写存储、不写日志。 */
  raw: unknown;
}

export interface DossierComment {
  sourceRef: string;
  parentSourceRef: string | null;
  relation: 'ROOT' | 'REPLY';
  body: string | null;
  likeCount: number | null;
}

export interface DossierCommentPage {
  items: DossierComment[];
  total: number | null;
  nextCursor: string | null;
}

export interface RelayerReceipt {
  scopeCount: number;
  updated: number;
  alreadyCurrent: number;
  skipped: number;
  needsOcr: number;
}
