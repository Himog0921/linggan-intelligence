import type { DossierMedia } from './dossierTypes';
import { purposeLabel } from './evidencePresent';
import type { Fragment, Inspector, MediaSlot } from './evidenceTypes';

/**
 * 轨道与查看器共用的「本机媒体对象」列表。作者头像不在作品自己的序列里（它属于作者），所以轨道不含头像，
 * 台账含头像。顺序按生产方自己的采集序号——系统里唯一真实持有的顺序；`displayOrderState` 在上游是 UNKNOWN，
 * 所以界面只说「采集顺序」，不声称平台排列。
 *
 * 同一份文件只算一个对象：小红书的封面就是正文第 1 张图，采集时会落成封面与正文两个槽位、字节指纹相同
 * （2026-10-23 线上 2,065 篇里 2,060 篇如此）。轨道按指纹合并、标出它同时担任的角色；槽位本身一个不删，
 * 逐槽位的台账见 `mediaSlotLedger`。与图片识别选图同一规则：任一别名是封面，这张图就算封面。
 */
export interface ViewerObject {
  index: number;
  purpose: string;
  /** 卡片与查看器上的角色名；同一张图担任多个角色时写全，例如「封面 · 正文第 1 张」。 */
  label: string;
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
  /** 这个对象代表的全部槽位（同一份文件的别名），第一个是 `slotKey`。 */
  slotKeys: readonly string[];
  derivedText: string | null;
  /** 原声来自这个槽位（检索命中的那张图）。 */
  isMatch: boolean;
  /** 有本地句柄但服务器没有标记为可内联。 */
  inlineBlocked: boolean;
  mime: string;
}

const TEXT_KINDS = new Set(['ocr_text', 'frame_ocr_text', 'asr_text']);

interface DerivedFacts {
  state: string | null;
  text: string | null;
}

function derivedBySlot(inspector: Inspector): Map<string, DerivedFacts> {
  const result = new Map<string, DerivedFacts>();
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

function sortedSlots(inspector: Inspector, includeAvatar: boolean): MediaSlot[] {
  return inspector.mediaSlots
    .filter(slot => includeAvatar || slot.purpose !== 'author_avatar')
    .sort((left, right) => ordinalOf(left) - ordinalOf(right) || (left.slotKey ?? '').localeCompare(right.slotKey ?? ''));
}

/** 同一指纹的槽位归为一组；没有指纹（还没取得字节）的槽位各自成组，不猜它们是不是同一张。 */
function groupByContent(slots: readonly MediaSlot[]): MediaSlot[][] {
  const groups: MediaSlot[][] = [];
  const byContent = new Map<string, MediaSlot[]>();
  for (const slot of slots) {
    const known = slot.blobSha256 === null ? undefined : byContent.get(slot.blobSha256);
    if (known) {
      known.push(slot);
      continue;
    }
    const group = [slot];
    if (slot.blobSha256 !== null) byContent.set(slot.blobSha256, group);
    groups.push(group);
  }
  return groups;
}

function roleLabel(slot: MediaSlot, merged: boolean, fallbackOrdinal: number): string {
  if (merged && slot.purpose === 'body_image') return `正文第 ${slot.producerOrdinal ?? slot.relationshipOrdinal ?? fallbackOrdinal} 张`;
  return purposeLabel(slot.purpose);
}

function groupLabel(group: readonly MediaSlot[], fallbackOrdinal: number): string {
  const merged = group.length > 1;
  return [...new Set(group.map(slot => roleLabel(slot, merged, fallbackOrdinal)))].join(' · ');
}

function inlineUrl(slot: MediaSlot): string | null {
  return slot.deliveryState === 'INLINE_SAFE' && slot.mime.startsWith('image/') ? slot.assetUrl : null;
}

export function mediaObjects(inspector: Inspector, fragment: Pick<Fragment, 'slotKey'> | null): MediaObject[] {
  const derived = derivedBySlot(inspector);
  return groupByContent(sortedSlots(inspector, false)).map((group, index) => {
    // 封面优先作为代表：与图片识别选图一致（任一别名是封面即算封面）。
    const slot = group.find(member => member.purpose === 'cover') ?? group[0]!;
    const members = [slot, ...group.filter(member => member !== slot)];
    const slotKeys = members.flatMap(member => (member.slotKey ? [member.slotKey] : []));
    const facts = slotKeys.map(key => derived.get(key)).filter((value): value is DerivedFacts => value !== undefined);
    const text = facts.find(value => value.text !== null) ?? facts.find(value => value.state !== null) ?? { state: null, text: null };
    const assetUrl = members.map(inlineUrl).find(url => url !== null) ?? null;
    return {
      index,
      slot,
      slotKey: slot.slotKey,
      slotKeys,
      purpose: slot.purpose,
      label: groupLabel(members, index + 1),
      ordinal: slot.producerOrdinal ?? slot.relationshipOrdinal ?? index + 1,
      objectRef: slot.materializationRef,
      bytesState: slot.bytesState,
      derivedState: text.state,
      derivedText: text.text,
      isMatch: Boolean(fragment?.slotKey && slotKeys.includes(fragment.slotKey)),
      assetUrl,
      inlineBlocked: assetUrl === null && members.some(member => member.hasHandle && member.deliveryState !== 'INLINE_SAFE'),
      mime: slot.mime,
    };
  });
}

/** 逐槽位的技术台账：一个槽位一行，含作者头像；同一份文件的两个槽位在这里仍是两行。 */
export function mediaSlotLedger(inspector: Inspector, fragment: Pick<Fragment, 'slotKey'> | null): MediaObject[] {
  const derived = derivedBySlot(inspector);
  return sortedSlots(inspector, true).map((slot, index) => {
    const text = (slot.slotKey ? derived.get(slot.slotKey) : undefined) ?? { state: null, text: null };
    return {
      index,
      slot,
      slotKey: slot.slotKey,
      slotKeys: slot.slotKey ? [slot.slotKey] : [],
      purpose: slot.purpose,
      label: purposeLabel(slot.purpose),
      ordinal: slot.producerOrdinal ?? slot.relationshipOrdinal ?? index + 1,
      objectRef: slot.materializationRef,
      bytesState: slot.bytesState,
      derivedState: text.state,
      derivedText: text.text,
      isMatch: Boolean(fragment?.slotKey && fragment.slotKey === slot.slotKey),
      assetUrl: inlineUrl(slot),
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
    label: purposeLabel('body_image'),
    ordinal: media.slotOrdinal ?? 1,
    objectRef: media.originalMaterializationRef,
    bytesState: 'ACQUIRED',
    derivedState: media.state,
    assetUrl: media.originalUrl,
  };
}
