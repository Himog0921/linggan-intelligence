import type { DossierMedia } from './dossierTypes';
import type { Fragment, Inspector, MediaSlot } from './evidenceTypes';

/**
 * 轨道与查看器共用的「本机媒体对象」列表。作者头像不在作品自己的序列里（它属于作者），所以轨道不含头像，
 * 台账含头像。顺序按生产方自己的采集序号——系统里唯一真实持有的顺序；`displayOrderState` 在上游是 UNKNOWN，
 * 所以界面只说「采集顺序」，不声称平台排列。
 */
export interface ViewerObject {
  index: number;
  purpose: string;
  ordinal: number;
  objectRef: string | null;
  bytesState: string;
  /** 这一槽位的图片文字／转录处理状态；没有处理记录是 null（界面写「文字未处理」）。 */
  derivedState: string | null;
  /** 可内联显示的受控本地图片；视频、实况动态部分与不安全类型没有。 */
  assetUrl: string | null;
}

export interface MediaObject extends ViewerObject {
  slot: MediaSlot;
  slotKey: string | null;
  derivedText: string | null;
  /** 原声来自这个槽位（检索命中的那张图）。 */
  isMatch: boolean;
  /** 有本地句柄但服务器没有标记为可内联。 */
  inlineBlocked: boolean;
  mime: string;
}

const TEXT_KINDS = new Set(['ocr_text', 'frame_ocr_text', 'asr_text']);

function derivedBySlot(inspector: Inspector): Map<string, { state: string | null; text: string | null }> {
  const result = new Map<string, { state: string | null; text: string | null }>();
  for (const derivative of inspector.derivatives) {
    if (!derivative.slotKey) continue;
    const existing = result.get(derivative.slotKey) ?? { state: null, text: null };
    if (derivative.kind !== null && TEXT_KINDS.has(derivative.kind)) {
      existing.state = derivative.state;
      if (derivative.displayText && derivative.displayText.trim()) existing.text = derivative.displayText;
    }
    result.set(derivative.slotKey, existing);
  }
  return result;
}

function ordinalOf(slot: MediaSlot): number {
  return slot.producerOrdinal ?? slot.relationshipOrdinal ?? 0;
}

export function mediaObjects(inspector: Inspector, fragment: Pick<Fragment, 'slotKey'> | null, includeAvatar = false): MediaObject[] {
  const derived = derivedBySlot(inspector);
  return inspector.mediaSlots
    .filter(slot => includeAvatar || slot.purpose !== 'author_avatar')
    .sort((left, right) => ordinalOf(left) - ordinalOf(right) || (left.slotKey ?? '').localeCompare(right.slotKey ?? ''))
    .map((slot, index) => {
      const text = (slot.slotKey ? derived.get(slot.slotKey) : undefined) ?? { state: null, text: null };
      return {
        index,
        slot,
        slotKey: slot.slotKey,
        purpose: slot.purpose,
        ordinal: slot.producerOrdinal ?? slot.relationshipOrdinal ?? index + 1,
        objectRef: slot.materializationRef,
        bytesState: slot.bytesState,
        derivedState: text.state,
        derivedText: text.text,
        isMatch: Boolean(fragment?.slotKey && fragment.slotKey === slot.slotKey),
        assetUrl: slot.deliveryState === 'INLINE_SAFE' && slot.mime.startsWith('image/') ? slot.assetUrl : null,
        inlineBlocked: slot.hasHandle && slot.deliveryState !== 'INLINE_SAFE',
        mime: slot.mime,
      };
    });
}

/** 轨道卡与查看器上的序号：`NN / MM`。 */
export function positionText(index: number, total: number): string {
  return `${String(index + 1).padStart(2, '0')} / ${String(total).padStart(2, '0')}`;
}

/** 底稿里「放大查看原图」用的单图对象：同一个查看器，单图模式，不能前后翻。 */
export function singleViewerObject(media: DossierMedia): ViewerObject {
  return {
    index: 0,
    purpose: 'body_image',
    ordinal: media.slotOrdinal ?? 1,
    objectRef: media.originalMaterializationRef,
    bytesState: 'ACQUIRED',
    derivedState: media.state,
    assetUrl: media.originalUrl,
  };
}
