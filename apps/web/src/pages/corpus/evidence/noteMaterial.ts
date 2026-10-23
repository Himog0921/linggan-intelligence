import { ApiError, requestJson } from '../../../shared/api';
import { bool, decode, list, num, obj, ShapeError, str, strList, type Json } from '../../../shared/decode';
import { API_ROOT } from './evidencePaths';

export const MATERIAL_FIELDS = ['title', 'cover_headline', 'body', 'tags', 'ocr', 'transcript'] as const;
export type MaterialField = typeof MATERIAL_FIELDS[number];
export const COLUMN_LABELS = {
  available: '已取得内容', not_provided: '来源未提供', not_acquired: '尚未取得',
  waiting: '等待处理', known_empty: '本次来源明确为空', terminal_failure: '本次处理已终止',
  restricted: '材料受限', not_adopted: '未纳入本次输入', not_connected: '本期未接入',
  needs_relayer: '需要重新分层', invalid_source: '来源异常',
} as const;
export type ColumnState = keyof typeof COLUMN_LABELS;

const REASONS: Readonly<Record<string, string>> = {
  waiting_detail_within_24h: '详情仍在取得宽限内', ocr_processing_pending: '图片文字处理未结束',
  transcript_processing_pending: '转写处理未结束', ocr_needs_relayer: '图片文字需要当前分层',
  ocr_machine_preview_unverified: '图片文字仅有机器预览，尚未人工采用',
  tags_projection_pending: '话题标签投影待补', qualified_text_unavailable: '尚无符合条件的文字',
  media_processing_pending: '媒体处理未结束', media_work_state_unavailable: '当前媒体作业证据不足',
  media_processing_terminal: '本次媒体处理已终止', media_processing_not_applicable: '本次媒体处理不适用',
  processor_reported_empty: '本次处理明确未发现文字或语音', source_restricted: '来源已受限',
  machine_preview_unverified: '机器预览尚未人工采用，不会进入本次输入',
  current_layer_required: '需要当前分层版本', detail_not_acquired: '尚未取得详情',
  source_field_absent: '来源未提供此字段', source_field_invalid: '来源字段异常',
  source_provided_empty_array: '本次来源明确提供空列表',
};

/** Retain only a bounded machine code, never the rejected response or plaintext. */
export class MaterialContractError extends ApiError {
  readonly diagnostic: string;
  constructor(field: string, value: string) {
    super({ kind: 'invalid_response', status: 200 });
    this.diagnostic = `${field}: ${/^[a-zA-Z0-9_.:-]{1,160}$/.test(value) ? value : 'unsupported_value'}`;
  }
}

/** Slot/input-scope prefix is evidence location, never part of a guessed status. */
export function materialReason(reason: string): string {
  const code = reason.split(':').at(-1) ?? '';
  const label = Object.hasOwn(REASONS, code) ? REASONS[code] : undefined;
  if (!label) throw new MaterialContractError('reason', reason);
  return reason === code ? label : `${label}（位置 ${reason.slice(0, -(code.length + 1))}）`;
}

export interface MaterialColumn { field: MaterialField; state: ColumnState; reasons: string[] }
export interface MaterialFragment {
  citationId: string; field: MaterialField; logicalPosition: string; sourceRef: string;
  sourceVersion: string; start: number; end: number; textHash: string; text: string;
}
export interface NoteMaterial {
  domainRef: string; workRef: string; asOf: string; dossierRevision: string;
  materialFingerprint: string; provenanceHash: string; snapshotIdentity: string;
  ready: boolean; notReadyReasons: string[]; columns: MaterialColumn[]; fragments: MaterialFragment[];
  layerVersion: string; processingScope: { included: string[]; contextOnly: string[]; notConnected: string[] };
}

function materialField(value: string): MaterialField {
  const field = MATERIAL_FIELDS.find(field => field === value);
  if (!field) throw new ShapeError('field');
  return field;
}
function requiredStrings(root: Json, name: string): string[] {
  if (!Array.isArray(root[name])) throw new ShapeError(name);
  return strList(root, name);
}
function reasons(root: Json, name: string): string[] {
  const values = requiredStrings(root, name);
  values.forEach(materialReason);
  return values;
}
function nonempty(root: Json, name: string): string {
  const value = str(root, name);
  if (!value) throw new ShapeError(name);
  return value;
}
function hash(root: Json, name: string): string {
  const value = str(root, name);
  if (!/^[a-f0-9]{64}$/.test(value)) throw new ShapeError(name);
  return value;
}
function scopeValues(root: Json, name: string, allowed: readonly string[]): string[] {
  const values = requiredStrings(root, name);
  const unknown = values.find(value => !allowed.includes(value));
  if (unknown !== undefined) throw new MaterialContractError(name, unknown);
  if (new Set(values).size !== values.length) throw new ShapeError(name);
  return values;
}

/** Atomic decoding: a new state/contract/reason cannot expose plaintext from this or an earlier read. */
export function parseNoteMaterial(value: unknown, work: string, domain: string): NoteMaterial {
  return decode(value, root => {
    if (str(root, 'contract') !== 'note-material-input.v1' || str(root, 'fingerprintContract') !== 'note-material-fingerprint.v1') throw new ShapeError('contract');
    if (str(root, 'workRef').toLowerCase() !== work.toLowerCase() || str(root, 'domainRef').toLowerCase() !== domain.toLowerCase()) throw new ShapeError('identity');
    const columns = list(root, 'columns', item => {
      const state = str(item, 'state');
      const known = Object.keys(COLUMN_LABELS).find(key => key === state) as ColumnState | undefined;
      if (!known) throw new MaterialContractError('state', state);
      return { field: materialField(str(item, 'field')), state: known, reasons: reasons(item, 'reasons') };
    });
    if (columns.length !== MATERIAL_FIELDS.length || new Set(columns.map(column => column.field)).size !== MATERIAL_FIELDS.length) throw new ShapeError('columns');
    const fragments = list(root, 'fragments', item => {
      const start = num(item, 'start'); const end = num(item, 'end'); const text = str(item, 'text');
      if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || end - start !== Array.from(text).length) throw new ShapeError('range');
      return {
        citationId: nonempty(item, 'citationId'), field: materialField(str(item, 'field')),
        logicalPosition: nonempty(item, 'logicalPosition'), sourceRef: nonempty(item, 'sourceRef'),
        sourceVersion: nonempty(item, 'sourceVersion'), start, end, textHash: hash(item, 'textHash'), text,
      };
    });
    if (new Set(fragments.map(fragment => fragment.citationId)).size !== fragments.length) throw new ShapeError('citations');
    const scope = obj(root, 'processingScope');
    return {
      domainRef: domain, workRef: work, asOf: nonempty(root, 'asOf'), dossierRevision: nonempty(root, 'dossierRevision'),
      materialFingerprint: hash(root, 'materialFingerprint'), provenanceHash: hash(root, 'provenanceHash'), snapshotIdentity: hash(root, 'snapshotIdentity'),
      ready: bool(root, 'ready'), notReadyReasons: reasons(root, 'notReadyReasons'), columns, fragments,
      layerVersion: nonempty(root, 'layerVersion'), processingScope: {
        included: scopeValues(scope, 'included', ['image_ocr', 'asr']),
        contextOnly: scopeValues(scope, 'contextOnly', ['comment_image', 'author_avatar', 'thumbnail', 'audio_extract']),
        notConnected: scopeValues(scope, 'notConnected', ['video_frame_ocr']),
      },
    };
  });
}

export async function fetchNoteMaterial(work: string, domain: string, signal: AbortSignal): Promise<NoteMaterial> {
  const value = await requestJson(`${API_ROOT}/${encodeURIComponent(work)}/research-dossier/note-material?domain=${encodeURIComponent(domain)}`, { signal });
  return parseNoteMaterial(value, work, domain);
}
