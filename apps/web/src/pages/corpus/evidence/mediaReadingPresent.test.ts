import { describe, expect, it } from 'vitest';
import { mediaReadingLabels } from './mediaReadingPresent';

const slots = (...purposes: string[]) => purposes.map(purpose => ({ purpose }));

describe('作品媒体的转录归属', () => {
  it('实况图片只保留静态图，不显示动态视频计数推导的转录', () => {
    expect(mediaReadingLabels(slots('cover', 'body_image', 'live_photo'), { videoCount: 6 })).toEqual({
      mediaTitle: '图组', transcriptLabel: null,
    });
  });

  it('历史动态视频计数不恢复实况图的转录入口', () => {
    for (const videoCount of [0, 1, 6]) {
      expect(mediaReadingLabels(slots('live_photo'), { videoCount }).transcriptLabel).toBe(null);
    }
  });

  it('普通视频与混合媒体按实际来源表达', () => {
    expect(mediaReadingLabels(slots('cover', 'video'), { videoCount: 1 })).toEqual({
      mediaTitle: '视频与图片', transcriptLabel: '视频转录',
    });
    expect(mediaReadingLabels(slots('video', 'live_photo'), { videoCount: 2 }).transcriptLabel).toBe('视频转录');
  });

  it('纯静态图片不出现转录；详情缺失时不猜测动态素材种类', () => {
    expect(mediaReadingLabels(slots('cover', 'body_image'), { videoCount: 0 }).transcriptLabel).toBeNull();
    expect(mediaReadingLabels([], { videoCount: 1 })).toEqual({ mediaTitle: '图组', transcriptLabel: '转录' });
  });
});
