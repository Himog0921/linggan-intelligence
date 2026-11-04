import type { ListMedia, MediaSlot } from './evidenceTypes';

/** A motion container can belong to a live photo; it does not make the work a video note. */
export function mediaReadingLabels(
  slots: readonly Pick<MediaSlot, 'purpose'>[],
  media: Pick<ListMedia, 'videoCount'>,
): { mediaTitle: string; transcriptLabel: string | null } {
  const video = slots.some(slot => slot.purpose === 'video');
  const livePhoto = slots.some(slot => slot.purpose === 'live_photo');
  if (livePhoto && !video) return { mediaTitle: '图组', transcriptLabel: null };
  if (video) return { mediaTitle: '视频与图片', transcriptLabel: '视频转录' };
  // Incomplete inspector slots must not invent a video/live-photo classification.
  return { mediaTitle: '图组', transcriptLabel: media.videoCount > 0 ? '转录' : null };
}
