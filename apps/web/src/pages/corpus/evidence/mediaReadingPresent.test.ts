import { describe, expect, it } from 'vitest';
import { mediaReadingLabels } from './mediaReadingPresent';

const slots = (...purposes: string[]) => purposes.map(purpose => ({ purpose }));

describe('作品媒体的转录归属', () => {
  it('实况图片的预排任务表达音轨检查，不把图文笔记称为视频', () => {
    expect(mediaReadingLabels(slots('cover', 'body_image', 'live_photo'), { videoCount: 6 }, 'QUEUED')).toEqual({
      mediaTitle: '图组', transcriptLabel: '实况图片音轨检查',
    });
  });

  it('已取得或执行中的实况转录仍保留，不把实况图片一律排除', () => {
    for (const state of ['ACQUIRED', 'PARTIAL', 'PROCESSING', 'FAILED', 'UNKNOWN', undefined]) {
      expect(mediaReadingLabels(slots('live_photo'), { videoCount: 1 }, state).transcriptLabel).toBe('实况图片转录');
    }
  });

  it('普通视频与混合媒体按实际来源表达', () => {
    expect(mediaReadingLabels(slots('cover', 'video'), { videoCount: 1 }, 'QUEUED')).toEqual({
      mediaTitle: '视频与图片', transcriptLabel: '视频转录',
    });
    expect(mediaReadingLabels(slots('video', 'live_photo'), { videoCount: 2 }, 'QUEUED').transcriptLabel).toBe('视频与实况图片转录');
  });

  it('纯静态图片不出现转录；详情缺失时不猜测动态素材种类', () => {
    expect(mediaReadingLabels(slots('cover', 'body_image'), { videoCount: 0 }, 'UNKNOWN').transcriptLabel).toBeNull();
    expect(mediaReadingLabels([], { videoCount: 1 }, 'QUEUED')).toEqual({ mediaTitle: '图组', transcriptLabel: '转录' });
  });
});
