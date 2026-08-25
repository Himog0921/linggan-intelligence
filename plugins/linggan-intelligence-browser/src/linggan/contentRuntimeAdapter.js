import { sendToBackground } from '../shared/messaging.js';
import { LINGGAN_RUNTIME_ACTION } from './runtimeActions.js';
import {
  createManualRuntimeTask,
  packageAuthorProfile,
  packageBatchCheckpoint,
  packageComments,
  packageContentDetail,
  packageDiscovery,
  packageMediaSlots,
  packageReplies,
} from './producerRuntime.js';

function pageTypeFor(platform, kind) {
  if (kind === 'author_profile') return 'profile';
  if (kind === 'discovery_search') return 'search_results';
  return platform === 'douyin' ? 'detail' : 'note_detail';
}

function taskFor(platform, capability, target, options = {}) {
  return createManualRuntimeTask({
    platform, pageType: pageTypeFor(platform, capability), target,
    capabilitiesRequested: [capability], maximumQuota: options.maximumQuota ?? 1,
    commentLimit: options.commentLimit ?? 'not_requested', acquireMedia: options.acquireMedia ?? 'not_requested',
    stopConditions: options.stopConditions || ['manual_stop', 'maximum_quota'],
  });
}

export function createLingganContentRuntime({ platform } = {}) {
  async function submit(taskSpec, capturePackage) {
    const response = await sendToBackground(LINGGAN_RUNTIME_ACTION.SUBMIT_CAPTURE_PACKAGE, { taskSpec, capturePackage }, { timeoutMs: 5000 });
    if (!response?.success) throw new Error(response?.message || 'Linggan 未能将采集结果写入本机待交付队列');
    return { ...response, taskSpec };
  }

  return {
    async submitDiscovery(cards, { query = '', authorExternalId = '', surface = 'current_visible_surface' } = {}) {
      const packageValue = packageDiscovery({ platform, cards, query, authorExternalId, surface });
      const capability = authorExternalId ? 'profile_discovery' : 'discovery_search';
      const target = authorExternalId
        ? { authorExternalId: String(authorExternalId), surface }
        : { query: String(query || ''), surface };
      return submit(taskFor(platform, capability, target, {
        maximumQuota: Math.max(1, packageValue.records.length),
        stopConditions: ['current_surface_read_once', 'maximum_quota'],
      }), packageValue);
    },
    async submitContentDetail(note) {
      const packageValue = packageContentDetail({ platform, note });
      return submit(taskFor(platform, 'content_detail', { contentExternalId: String(note?.noteId || note?.id || '') }, { acquireMedia: 'slots' }), packageValue);
    },
    async submitComments(result, noteId, settings = {}) {
      const packageValue = packageComments({ platform, result, noteId });
      const comments = await submit(
        taskFor(platform, 'comments', { contentExternalId: String(noteId || '') }, { commentLimit: settings.maxTotal ?? 'not_requested' }),
        packageValue,
      );
      const repliesPackage = packageReplies({ platform, result, noteId });
      if (repliesPackage.records.length === 0) return comments;
      const replies = await submit(
        taskFor(platform, 'replies', { contentExternalId: String(noteId || '') }, { commentLimit: settings.maxTotal ?? 'not_requested' }),
        repliesPackage,
      );
      return { ...comments, replies };
    },
    async submitAuthor(author) {
      const packageValue = packageAuthorProfile({ platform, author });
      return submit(taskFor(platform, 'author_profile', { authorExternalId: String(author?.userId || author?.id || '') }), packageValue);
    },
    async submitMediaSlots(note) {
      const packageValue = packageMediaSlots({ platform, note });
      const taskSpec = taskFor(platform, 'media_slots', { contentExternalId: String(note?.noteId || note?.id || '') }, { acquireMedia: 'bytes' });
      return submit(taskSpec, packageValue);
    },
    async acquireMediaSlots(note) {
      const packageValue = packageMediaSlots({ platform, note });
      const taskSpec = taskFor(platform, 'media_slots', { contentExternalId: String(note?.noteId || note?.id || '') }, { acquireMedia: 'bytes' });
      const response = await sendToBackground(LINGGAN_RUNTIME_ACTION.SUBMIT_MEDIA_SLOTS, { taskSpec, capturePackage: packageValue }, { timeoutMs: 5000 });
      if (!response?.success) throw new Error(response?.message || 'Linggan 未能将媒体下载加入本机队列');
      return { ...response, taskSpec, capturePackage: packageValue, total: packageValue.records.length, success: 0, failed: 0, queued: true };
    },
    async submitBatchCheckpoint(kind, progress) {
      const packageValue = packageBatchCheckpoint({ platform, kind, progress });
      return submit(taskFor(platform, 'batch_checkpoint', { taskType: kind }, { maximumQuota: Math.max(1, Number(progress?.total || 1)) }), packageValue);
    },
  };
}
