import test from 'node:test';
import assert from 'node:assert/strict';
import { createNoteMediaDownloadService } from '../src/content/noteMediaDownload.js';
import { mediaAssetStore } from '../src/db/mediaAssetStore.js';

function installBrowserMocks() {
  const listeners = new Map();
  const downloads = [];
  const objectUrls = [];
  globalThis.CustomEvent = class CustomEvent {
    constructor(type, init = {}) {
      this.type = type;
      this.detail = init.detail;
    }
  };
  globalThis.URL = {
    createObjectURL(blob) {
      const url = `blob:mock-${objectUrls.length + 1}`;
      objectUrls.push({ url, blob });
      return url;
    },
    revokeObjectURL() {},
  };
  globalThis.window = {
    location: {
      href: 'https://example.com/dashboard',
      host: 'example.com',
      protocol: 'https:',
      origin: 'https://example.com',
    },
    addEventListener(type, handler) {
      listeners.set(type, handler);
    },
    removeEventListener(type) {
      listeners.delete(type);
    },
    dispatchEvent(event) {
      const handler = listeners.get(event?.type);
      if (handler) handler(event);
      return true;
    },
  };
  globalThis.document = {
    body: {
      appendChild() {},
    },
    createElement() {
      return {
        style: {},
        click() {
          downloads.push({
            href: this.href,
            download: this.download,
          });
        },
        remove() {},
        set src(value) {
          this._src = value;
        },
        get src() {
          return this._src;
        },
      };
    },
  };
  return {
    downloads,
    objectUrls,
    listeners,
  };
}

function uninstallBrowserMocks() {
  delete globalThis.window;
  delete globalThis.document;
  delete globalThis.CustomEvent;
  delete globalThis.URL;
  delete globalThis.fetch;
  delete globalThis.XMLHttpRequest;
}

test('downloadNoteMediaFromRecord refreshes douyin note before first download when queue is empty', async () => {
  installBrowserMocks();
  const originalBulkUpsert = mediaAssetStore.bulkUpsert;
  const assetWrites = [];
  mediaAssetStore.bulkUpsert = async (assets) => {
    assetWrites.push(assets);
  };

  const noteUpdates = [];
  const bgCalls = [];

  try {
    const service = createNoteMediaDownloadService({
      MSG: { DOWNLOAD_MEDIA_FILE: 'downloadMediaFile' },
      noteStore: {
        async updateById(noteId, patch) {
          noteUpdates.push({ noteId, patch });
        },
      },
      sendToBackground: async (action, payload) => {
        bgCalls.push({ action, payload });
        return {
          success: true,
          quality: 'HD',
          sourceUrl: payload.candidates?.[0] || '',
          via: 'chrome.downloads',
        };
      },
      collectNote: async () => null,
      loadDouyinRuntime: async () => ({
        extractDouyinContentId: () => '',
        collectDouyinVideo: async () => null,
        refreshDouyinNoteMediaById: async (note) => ({
          ...note,
          platform: 'douyin',
          noteId: 'dy_123',
          contentId: 'dy_123',
          platformContentId: '123',
          title: '新视频',
          videoDownloadUrl: 'https://cdn.example/fresh.mp4',
          videoPlayUrl: 'https://cdn.example/fresh.mp4',
          videoStreams: [{ url: 'https://cdn.example/fresh.mp4', bitrate: 1 }],
        }),
      }),
      extractNoteId: () => '',
    });

    const summary = await service.downloadNoteMediaFromRecord({
      platform: 'douyin',
      noteId: 'dy_123',
      contentId: 'dy_123',
      platformContentId: '123',
      title: '旧视频',
    });

    assert.equal(summary.total, 1);
    assert.equal(summary.success, 1);
    assert.equal(summary.failed, 0);
    assert.equal(summary.refreshed, true);
    assert.equal(bgCalls.length, 1);
    assert.equal(bgCalls[0].payload.candidates[0], 'https://cdn.example/fresh.mp4');
    assert.ok(noteUpdates.some((entry) => entry.patch?.videoDownloadUrl === 'https://cdn.example/fresh.mp4'));
    assert.ok(assetWrites.length >= 2);
  } finally {
    mediaAssetStore.bulkUpsert = originalBulkUpsert;
    uninstallBrowserMocks();
  }
});

test('downloadNoteMediaFromRecord retries douyin video download after refreshing stale media urls', async () => {
  installBrowserMocks();
  const originalBulkUpsert = mediaAssetStore.bulkUpsert;
  const assetWrites = [];
  mediaAssetStore.bulkUpsert = async (assets) => {
    assetWrites.push(assets);
  };

  const noteUpdates = [];
  const bgCalls = [];
  globalThis.fetch = async () => ({ ok: false, status: 403 });

  try {
    const service = createNoteMediaDownloadService({
      MSG: { DOWNLOAD_MEDIA_FILE: 'downloadMediaFile' },
      noteStore: {
        async updateById(noteId, patch) {
          noteUpdates.push({ noteId, patch });
        },
      },
      sendToBackground: async (action, payload) => {
        bgCalls.push({ action, payload });
        if (bgCalls.length === 1) {
          return { success: false, error: 'expired_url' };
        }
        return {
          success: true,
          quality: 'HD',
          sourceUrl: payload.candidates?.[0] || '',
          via: 'chrome.downloads',
        };
      },
      collectNote: async () => null,
      loadDouyinRuntime: async () => ({
        extractDouyinContentId: () => '',
        collectDouyinVideo: async () => null,
        refreshDouyinNoteMediaById: async (note) => ({
          ...note,
          platform: 'douyin',
          noteId: 'dy_123',
          contentId: 'dy_123',
          platformContentId: '123',
          title: '新视频',
          videoDownloadUrl: 'https://cdn.example/fresh.mp4',
          videoPlayUrl: 'https://cdn.example/fresh.mp4',
          videoStreams: [{ url: 'https://cdn.example/fresh.mp4', bitrate: 1 }],
        }),
      }),
      extractNoteId: () => '',
    });

    const summary = await service.downloadNoteMediaFromRecord({
      platform: 'douyin',
      noteId: 'dy_123',
      contentId: 'dy_123',
      platformContentId: '123',
      title: '旧视频',
      videoDownloadUrl: 'https://cdn.example/stale.mp4',
      videoPlayUrl: 'https://cdn.example/stale.mp4',
      videoStreams: [{ url: 'https://cdn.example/stale.mp4', bitrate: 1 }],
    });

    assert.equal(summary.total, 1);
    assert.equal(summary.success, 1);
    assert.equal(summary.failed, 0);
    assert.equal(summary.refreshed, true);
    assert.equal(bgCalls.length, 2);
    assert.equal(bgCalls[0].payload.candidates[0], 'https://cdn.example/stale.mp4');
    assert.equal(bgCalls[1].payload.candidates[0], 'https://cdn.example/fresh.mp4');
    assert.ok(noteUpdates.some((entry) => entry.patch?.videoDownloadUrl === 'https://cdn.example/fresh.mp4'));
    assert.ok(assetWrites.length >= 3);
  } finally {
    mediaAssetStore.bulkUpsert = originalBulkUpsert;
    uninstallBrowserMocks();
  }
});

test('downloadNoteMediaFromRecord downloads xhs image note media directly', async () => {
  const browser = installBrowserMocks();
  const originalBulkUpsert = mediaAssetStore.bulkUpsert;
  mediaAssetStore.bulkUpsert = async () => {};

  const bgCalls = [];

  try {
    const service = createNoteMediaDownloadService({
      MSG: {
        DOWNLOAD_MEDIA_FILE: 'downloadMediaFile',
        FETCH_BINARY_AS_DATA_URL: 'fetchBinaryAsDataUrl',
      },
      noteStore: {
        async updateById() {},
      },
      sendToBackground: async (action, payload) => {
        bgCalls.push({ action, payload });
        assert.equal(action, 'downloadMediaFile');
        return {
          success: true,
          quality: 'HD',
          sourceUrl: payload.candidates?.[0] || '',
          candidateIndex: 0,
          via: 'chrome.downloads',
        };
      },
      collectNote: async () => null,
      loadDouyinRuntime: async () => ({
        extractDouyinContentId: () => '',
        collectDouyinVideo: async () => null,
        refreshDouyinNoteMediaById: async () => null,
      }),
      extractNoteId: () => '',
    });

    const summary = await service.downloadNoteMediaFromRecord({
      platform: 'xhs',
      noteId: 'xhs_note_1',
      contentId: 'xhs_note_1',
      title: '测试标题',
      imageCandidates: [
        ['https://sns-img-qc.xhscdn.com/1.jpg'],
        ['https://sns-img-qc.xhscdn.com/2.jpg'],
      ],
    });

    assert.equal(summary.total, 2);
    assert.equal(summary.success, 2);
    assert.equal(summary.failed, 0);
    assert.equal(summary.zipped, false);
    assert.equal(bgCalls.length, 2);
    assert.equal(bgCalls[0].payload.candidates[0], 'https://sns-img-qc.xhscdn.com/1.jpg');
    assert.equal(bgCalls[1].payload.candidates[0], 'https://sns-img-qc.xhscdn.com/2.jpg');
    assert.equal(bgCalls[0].payload.headers[0].name, 'Referer');
    assert.equal(browser.downloads.length, 0);
  } finally {
    mediaAssetStore.bulkUpsert = originalBulkUpsert;
    uninstallBrowserMocks();
  }
});

test('downloadNoteMediaFromRecord accepts object-shaped xhs image candidates from saved dashboard records', async () => {
  const browser = installBrowserMocks();
  const originalBulkUpsert = mediaAssetStore.bulkUpsert;
  mediaAssetStore.bulkUpsert = async () => {};

  const bgCalls = [];

  try {
    const service = createNoteMediaDownloadService({
      MSG: {
        DOWNLOAD_MEDIA_FILE: 'downloadMediaFile',
        FETCH_BINARY_AS_DATA_URL: 'fetchBinaryAsDataUrl',
      },
      noteStore: {
        async updateById() {},
      },
      sendToBackground: async (action, payload) => {
        bgCalls.push({ action, payload });
        assert.equal(action, 'downloadMediaFile');
        assert.equal(payload.candidates[0], 'https://sns-img.example.com/candidate-cover.jpg');
        return {
          success: true,
          quality: 'HD',
          sourceUrl: payload.candidates[0],
          candidateIndex: 0,
          via: 'chrome.downloads',
        };
      },
      collectNote: async () => null,
      loadDouyinRuntime: async () => ({
        extractDouyinContentId: () => '',
        collectDouyinVideo: async () => null,
        refreshDouyinNoteMediaById: async () => null,
      }),
      extractNoteId: () => '',
    });

    const summary = await service.downloadNoteMediaFromRecord({
      platform: 'xhs',
      noteId: 'xhs_saved_note_1',
      contentId: 'xhs_saved_note_1',
      title: '保存记录',
      images: ['https://sns-img.example.com/fallback-cover.jpg'],
      imageCandidates: [[{ url: 'https://sns-img.example.com/candidate-cover.jpg' }]],
    });

    assert.equal(summary.total, 1);
    assert.equal(summary.success, 1);
    assert.equal(summary.failed, 0);
    assert.equal(summary.zipped, false);
    assert.equal(bgCalls.length, 1);
    assert.equal(browser.downloads.length, 0);
  } finally {
    mediaAssetStore.bulkUpsert = originalBulkUpsert;
    uninstallBrowserMocks();
  }
});

test('downloadNoteMediaFromRecord honors selected xhs media types', async () => {
  installBrowserMocks();
  const originalBulkUpsert = mediaAssetStore.bulkUpsert;
  mediaAssetStore.bulkUpsert = async () => {};

  const bgCalls = [];

  try {
    const service = createNoteMediaDownloadService({
      MSG: {
        DOWNLOAD_MEDIA_FILE: 'downloadMediaFile',
        FETCH_BINARY_AS_DATA_URL: 'fetchBinaryAsDataUrl',
      },
      noteStore: {
        async updateById() {},
      },
      sendToBackground: async (action, payload) => {
        bgCalls.push({ action, payload });
        return {
          success: true,
          quality: 'HD',
          sourceUrl: payload.candidates?.[0] || '',
          candidateIndex: 0,
          via: 'chrome.downloads',
        };
      },
      collectNote: async () => null,
      loadDouyinRuntime: async () => ({
        extractDouyinContentId: () => '',
        collectDouyinVideo: async () => null,
        refreshDouyinNoteMediaById: async () => null,
      }),
      extractNoteId: () => '',
    });

    const summary = await service.downloadNoteMediaFromRecord({
      platform: 'xhs',
      noteId: 'xhs_rich_note_1',
      contentId: 'xhs_rich_note_1',
      title: '富媒体记录',
      type: 'video',
      cover: 'https://sns-img-qc.xhscdn.com/cover.jpg',
      imageCandidates: [
        ['https://sns-img-qc.xhscdn.com/image-1.jpg'],
        ['https://sns-img-qc.xhscdn.com/image-2.jpg'],
      ],
      livePhotoStreams: [{
        imageIndex: 2,
        candidates: ['https://sns-video-hw.xhscdn.com/live-2.mp4'],
      }],
      videoStreams: [{ url: 'https://sns-video-hw.xhscdn.com/video.mp4', bitrate: 1000 }],
    }, {
      mediaTypes: ['cover', 'live', 'video'],
    });

    assert.equal(summary.total, 3);
    assert.equal(summary.success, 3);
    assert.deepEqual(summary.details.map((item) => item.id), ['cover-1', 'live-2', 'video-1']);
    assert.equal(bgCalls.length, 3);
    assert.ok(bgCalls[0].payload.filename.endsWith('/封面.jpg'));
    assert.ok(bgCalls[1].payload.filename.endsWith('/Live_02.mp4'));
    assert.ok(bgCalls[2].payload.filename.endsWith('/富媒体记录.mp4'));
  } finally {
    mediaAssetStore.bulkUpsert = originalBulkUpsert;
    uninstallBrowserMocks();
  }
});

test('downloadNoteMediaFromRecord downloads only cover when cover is the only selected xhs media type', async () => {
  installBrowserMocks();
  const originalBulkUpsert = mediaAssetStore.bulkUpsert;
  mediaAssetStore.bulkUpsert = async () => {};

  const bgCalls = [];

  try {
    const service = createNoteMediaDownloadService({
      MSG: {
        DOWNLOAD_MEDIA_FILE: 'downloadMediaFile',
        FETCH_BINARY_AS_DATA_URL: 'fetchBinaryAsDataUrl',
      },
      noteStore: {
        async updateById() {},
      },
      sendToBackground: async (action, payload) => {
        bgCalls.push({ action, payload });
        return {
          success: true,
          quality: 'HD',
          sourceUrl: payload.candidates?.[0] || '',
          candidateIndex: 0,
          via: 'chrome.downloads',
        };
      },
      collectNote: async () => null,
      loadDouyinRuntime: async () => ({
        extractDouyinContentId: () => '',
        collectDouyinVideo: async () => null,
        refreshDouyinNoteMediaById: async () => null,
      }),
      extractNoteId: () => '',
    });

    const summary = await service.downloadNoteMediaFromRecord({
      platform: 'xhs',
      noteId: 'xhs_cover_only_1',
      contentId: 'xhs_cover_only_1',
      title: '只下封面',
      cover: 'https://sns-img-qc.xhscdn.com/cover.jpg',
      imageCandidates: [
        ['https://sns-img-qc.xhscdn.com/image-1.jpg'],
        ['https://sns-img-qc.xhscdn.com/image-2.jpg'],
        ['https://sns-img-qc.xhscdn.com/image-3.jpg'],
      ],
    }, {
      mediaTypes: ['cover'],
    });

    assert.equal(summary.total, 1);
    assert.equal(summary.success, 1);
    assert.deepEqual(summary.details.map((item) => item.id), ['cover-1']);
    assert.equal(bgCalls.length, 1);
    assert.ok(bgCalls[0].payload.filename.endsWith('/封面.jpg'));
  } finally {
    mediaAssetStore.bulkUpsert = originalBulkUpsert;
    uninstallBrowserMocks();
  }
});

test('downloadNoteMediaFromRecord falls back to page fetch for xhs images when background download fails', async () => {
  const browser = installBrowserMocks();
  const originalBulkUpsert = mediaAssetStore.bulkUpsert;
  mediaAssetStore.bulkUpsert = async () => {};
  globalThis.fetch = async () => ({
    ok: true,
    blob: async () => new Blob(['image-binary'], { type: 'image/jpeg' }),
  });

  try {
    const service = createNoteMediaDownloadService({
      MSG: {
        DOWNLOAD_MEDIA_FILE: 'downloadMediaFile',
        FETCH_BINARY_AS_DATA_URL: 'fetchBinaryAsDataUrl',
      },
      noteStore: {
        async updateById() {},
      },
      sendToBackground: async () => ({ success: false, error: 'all_candidates_failed' }),
      collectNote: async () => null,
      loadDouyinRuntime: async () => ({
        extractDouyinContentId: () => '',
        collectDouyinVideo: async () => null,
        refreshDouyinNoteMediaById: async () => null,
      }),
      extractNoteId: () => '',
    });

    const summary = await service.downloadNoteMediaFromRecord({
      platform: 'xhs',
      noteId: 'xhs_blob_fallback_1',
      contentId: 'xhs_blob_fallback_1',
      title: '兜底下载',
      imageCandidates: [['https://sns-img-qc.xhscdn.com/fallback.jpg']],
    });

    assert.equal(summary.total, 1);
    assert.equal(summary.success, 1);
    assert.equal(summary.failed, 0);
    assert.equal(summary.details[0].result.via, 'blob-fallback');
    assert.equal(browser.downloads.length, 1);
    assert.match(browser.downloads[0].download, /图_01\.jpg$/);
  } finally {
    mediaAssetStore.bulkUpsert = originalBulkUpsert;
    uninstallBrowserMocks();
  }
});

test('downloadNoteMediaFromRecord downloads douyin image notes through page-context fallback', async () => {
  const browser = installBrowserMocks();
  globalThis.window.location = {
    href: 'https://www.douyin.com/note/7590027857632480512',
    host: 'www.douyin.com',
    protocol: 'https:',
    origin: 'https://www.douyin.com',
  };
  globalThis.window.dispatchEvent = (event) => {
    if (event?.type === '__lgboom_page_download_req__') {
      const request = event.detail || {};
      setTimeout(() => {
        const responseHandler = browser.listeners.get('__lgboom_page_download_res__');
        responseHandler?.({
          detail: {
            requestId: request.requestId,
            ok: true,
            url: request.urls?.[0] || '',
          },
        });
      }, 0);
    }
    const handler = browser.listeners.get(event?.type);
    if (handler) handler(event);
    return true;
  };
  const originalBulkUpsert = mediaAssetStore.bulkUpsert;
  mediaAssetStore.bulkUpsert = async () => {};

  const bgCalls = [];

  try {
    const service = createNoteMediaDownloadService({
      MSG: {
        DOWNLOAD_MEDIA_FILE: 'downloadMediaFile',
        FETCH_BINARY_AS_DATA_URL: 'fetchBinaryAsDataUrl',
      },
      noteStore: {
        async updateById() {},
      },
      sendToBackground: async (action, payload) => {
        bgCalls.push({ action, payload });
        return { success: false, error: 'should_not_use_chrome_download_for_douyin' };
      },
      collectNote: async () => null,
      loadDouyinRuntime: async () => ({
        extractDouyinContentId: () => '',
        collectDouyinVideo: async () => null,
        refreshDouyinNoteMediaById: async () => null,
      }),
      extractNoteId: () => '',
    });

    const summary = await service.downloadNoteMediaFromRecord({
      platform: 'douyin',
      noteId: 'dy_7590027857632480512',
      contentId: 'dy_7590027857632480512',
      platformContentId: '7590027857632480512',
      title: '抖音图文',
      imageCandidates: [
        ['https://p3-sign.douyinpic.com/high.webp?x=1'],
      ],
    });

    assert.equal(summary.total, 1);
    assert.equal(summary.success, 1);
    assert.equal(summary.failed, 0);
    assert.equal(summary.details[0].result.via, 'page-context');
    assert.equal(summary.details[0].result.sourceUrl, 'https://p3-sign.douyinpic.com/high.webp?x=1');
    assert.equal(bgCalls.length, 0);
  } finally {
    mediaAssetStore.bulkUpsert = originalBulkUpsert;
    uninstallBrowserMocks();
  }
});

test('downloadNoteMediaFromRecord downloads xhs video directly and refreshes stale urls before retry', async () => {
  installBrowserMocks();
  globalThis.window.location = {
    href: 'https://www.xiaohongshu.com/explore/xhs_note_1',
    host: 'www.xiaohongshu.com',
    protocol: 'https:',
    origin: 'https://www.xiaohongshu.com',
  };
  const originalBulkUpsert = mediaAssetStore.bulkUpsert;
  const assetWrites = [];
  mediaAssetStore.bulkUpsert = async (assets) => {
    assetWrites.push(assets);
  };

  const noteUpdates = [];
  const bgCalls = [];

  try {
    const service = createNoteMediaDownloadService({
      MSG: {
        DOWNLOAD_MEDIA_FILE: 'downloadMediaFile',
        FETCH_BINARY_AS_DATA_URL: 'fetchBinaryAsDataUrl',
      },
      noteStore: {
        async updateById(noteId, patch) {
          noteUpdates.push({ noteId, patch });
        },
      },
      sendToBackground: async (action, payload) => {
        bgCalls.push({ action, payload });
        assert.equal(action, 'downloadMediaFile');
        if (String(payload.candidates?.[0] || '').includes('stale')) {
          return { success: false, error: 'expired_url' };
        }
        return {
          success: true,
          quality: 'HD',
          sourceUrl: payload.candidates?.[0] || '',
          via: 'chrome.downloads',
        };
      },
      collectNote: async () => ({
        platform: 'xhs',
        noteId: 'xhs_note_1',
        contentId: 'xhs_note_1',
        title: '视频标题',
        type: 'video',
        videoDownloadUrl: 'https://sns-video.example/fresh.mp4',
        videoStreams: [{ url: 'https://sns-video.example/fresh.mp4', bitrate: 2 }],
      }),
      loadDouyinRuntime: async () => ({
        extractDouyinContentId: () => '',
        collectDouyinVideo: async () => null,
        refreshDouyinNoteMediaById: async () => null,
      }),
      extractNoteId: () => 'xhs_note_1',
    });

    const summary = await service.downloadNoteMediaFromRecord({
      platform: 'xhs',
      noteId: 'xhs_note_1',
      contentId: 'xhs_note_1',
      title: '视频标题',
      type: 'video',
      url: 'https://www.xiaohongshu.com/explore/xhs_note_1',
      videoDownloadUrl: 'https://sns-video.example/stale.mp4',
      videoStreams: [{ url: 'https://sns-video.example/stale.mp4', bitrate: 1 }],
    });

    assert.equal(summary.total, 1);
    assert.equal(summary.success, 1);
    assert.equal(summary.failed, 0);
    assert.equal(summary.refreshed, true);
    assert.equal(summary.zipped, false);
    assert.equal(bgCalls.length, 2);
    assert.equal(bgCalls[0].payload.candidates[0], 'https://sns-video.example/stale.mp4');
    assert.equal(bgCalls[1].payload.candidates[0], 'https://sns-video.example/fresh.mp4');
    assert.ok(bgCalls.every((call) => call.action === 'downloadMediaFile'));
    assert.ok(noteUpdates.some((entry) => entry.patch?.videoDownloadUrl === 'https://sns-video.example/fresh.mp4'));
    assert.ok(assetWrites.length >= 3);
  } finally {
    mediaAssetStore.bulkUpsert = originalBulkUpsert;
    uninstallBrowserMocks();
  }
});
