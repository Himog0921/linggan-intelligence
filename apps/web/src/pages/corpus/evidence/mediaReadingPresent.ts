import type { ListMedia, MediaSlot } from './evidenceTypes';

/** A motion container can belong to a live photo; it does not make the work a video note. */
export function mediaReadingLabels(
  slots: readonly Pick<MediaSlot, 'purpose'>[],
  media: Pick<ListMedia, 'videoCount'>,
  transcriptState: string | undefined,
): { mediaTitle: string; transcriptLabel: string | null } {
  const video = slots.some(slot => slot.purpose === 'video');
  const livePhoto = slots.some(slot => slot.purpose === 'live_photo');
  if (livePhoto && !video) {
    return {
      mediaTitle: '图组',
      // Admission queues video containers before the worker checks for an audio stream.
      transcriptLabel: transcriptState === 'QUEUED' ? '实况图片音轨检查' : '实况图片转录',
    };
  }
  if (video) {
    return { mediaTitle: '视频与图片', transcriptLabel: livePhoto ? '视频与实况图片转录' : '视频转录' };
  }
  // Incomplete inspector slots must not invent a video/live-photo classification.
  return { mediaTitle: '图组', transcriptLabel: media.videoCount > 0 ? '转录' : null };
}
