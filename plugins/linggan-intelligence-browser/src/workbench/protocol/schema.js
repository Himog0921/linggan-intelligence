import { MSG } from '../../shared/constants.js';

export const WORKBENCH_PROTOCOL_VERSION = 'v1';

export const WORKBENCH_MESSAGE_TYPE = {
  EXECUTOR_HELLO: 'executor.hello',
  CAPABILITY_CHECK: 'capability.check',
  CAPABILITY_REPORT: 'capability.report',
  TASK_ENVELOPE: 'task.envelope',
  TASK_ACCEPTED: 'task.accepted',
  TASK_REJECTED: 'task.rejected',
  TASK_PROGRESS: 'task.progress',
  TASK_RESULT: 'task.result',
  TASK_FAILED: 'task.failed',
  TASK_CONTROL: 'task.control',
  TASK_RECORD: 'task.record',
};

export const REMOTE_TASK_TYPE = {
  XHS_BATCH_NOTES: 'xhs.batchNotes',
  XHS_BATCH_COMMENTS: 'xhs.batchComments',
  XHS_COLLECT_AUTHOR: 'xhs.collectAuthor',
  XHS_AUTHOR_NOTE_LINKS: 'xhs.authorNoteLinks',
  XHS_LIST_SCAN: 'xhs.list_scan',
  XHS_NOTE_FULL: 'xhs.note_full',
  XHS_COMMENT_SCAN: 'xhs.comment_scan',
  XHS_AUTHOR_PROFILE: 'xhs.author_profile',
  XHS_AUTHOR_LINKS: 'xhs.author_links',
  DOUYIN_BATCH_NOTES: 'douyin.batchNotes',
  DOUYIN_BATCH_COMMENTS: 'douyin.batchComments',
  DOUYIN_COLLECT_AUTHOR: 'douyin.collectAuthor',
  DOUYIN_SINGLE_COMMENTS: 'douyin.singleComments',
  DOUYIN_COMMENT_IMAGE_DOWNLOAD: 'douyin.commentImageDownload',
};

export const MONITOR_TASK_STRATEGY = {
  AUTHOR_BASELINE: 'author_baseline',
  AUTHOR_PATROL: 'author_patrol',
  KEYWORD_PATROL: 'keyword_patrol',
  DETAIL_PROBE: 'detail_probe',
  DEEP_COLLECT: 'deep_collect',
};

export const MONITOR_RECORD_MODE = {
  AUTHOR_PROFILE: 'author_profile',
  AUTHOR_SURFACE: 'author_surface',
  KEYWORD_SURFACE: 'keyword_surface',
  DETAIL_PROBE: 'detail_probe',
};

export const REMOTE_TASK_CONTROL_ACTION = {
  PAUSE: 'pause',
  RESUME: 'resume',
  STOP: 'stop',
  DELETE: 'delete',
};

export const WORKBENCH_TASK_EVENT_TYPE = {
  TASK_CLAIMED: 'task.claimed',
  TASK_PAGE_OPENED: 'task.page_opened',
  TASK_EXECUTION_STARTED: 'task.execution_started',
  TASK_FIRST_RECORD_SEEN: 'task.first_record_seen',
  TASK_PAGE_OPEN_FAILED: 'task.page_open_failed',
  TASK_LOGIN_REQUIRED: 'task.login_required',
  TASK_PLATFORM_RESTRICTED: 'task.platform_restricted',
  TASK_STARTED: 'task.started',
  TASK_RUNNING: 'task.running',
  TASK_HEARTBEAT: 'task.heartbeat',
  TASK_PROGRESS: 'task.progress',
  TASK_PARTIAL_RESULT: 'task.partial_result',
  TASK_CONTROL_REQUESTED: 'task.control_requested',
  TASK_CONTROL_APPLIED: 'task.control_applied',
  TASK_CONTROL_FAILED: 'task.control_failed',
  TASK_PAUSED: 'task.paused',
  TASK_RESUMED: 'task.resumed',
  TASK_STOPPING: 'task.stopping',
  TASK_STOPPED: 'task.stopped',
  TASK_COMPLETED: 'task.completed',
  TASK_SUCCEEDED: 'task.succeeded',
  TASK_RELEASED: 'task.released',
  TASK_FAILED: 'task.failed',
  TASK_DELETED: 'task.deleted',
  TASK_CAPABILITY_MISMATCH: 'task.capability_mismatch',
};

export const WORKBENCH_RECORD_TYPE = {
  NOTE: 'note',
  COMMENT: 'comment',
  AUTHOR: 'author',
  MEDIA: 'media',
};

export const WORKBENCH_EVENT_SOURCE = {
  WORKBENCH: 'workbench',
  PLUGIN: 'plugin',
  CONTENT: 'content',
};

export const REMOTE_EXECUTION_STATUS = {
  ACCEPTED: 'accepted',
  RUNNING: 'running',
  PAUSED: 'paused',
  STOPPING: 'stopping',
  DONE: 'done',
  FAILED: 'failed',
  CANCELED: 'canceled',
  REJECTED: 'rejected',
};

export const REMOTE_EXECUTION_STAGE = {
  CONTEXT_CHECK: 'context_check',
  DISCOVERING: 'discovering',
  COLLECTING: 'collecting',
  DOWNLOADING: 'downloading',
  PERSISTING: 'persisting',
  PACKAGING: 'packaging',
  FINALIZING: 'finalizing',
};

export const REMOTE_ERROR_CATEGORY = {
  CONTEXT: 'context',
  AUTH: 'auth',
  NETWORK: 'network',
  PLATFORM_BLOCK: 'platform_block',
  RATE_LIMIT: 'rate_limit',
  STORAGE: 'storage',
  DOWNLOAD: 'download',
  USER_CANCEL: 'user_cancel',
  INTERNAL: 'internal',
};

export const REMOTE_ERROR_CODE = {
  UNSUPPORTED_TASK_TYPE: 'unsupported_task_type',
  PLATFORM_MISMATCH: 'platform_mismatch',
  PAGE_TYPE_MISMATCH: 'page_type_mismatch',
  PAGE_TARGET_MISMATCH: 'page_target_mismatch',
  PAGE_CONTEXT_UNAVAILABLE: 'page_context_unavailable',
  PAGE_PERMISSION_DENIED: 'page_permission_denied',
  ERROR_PAGE: 'error_page',
  PLATFORM_SECURITY_CHALLENGE: 'platform_security_challenge',
  PLATFORM_BLOCKED: 'platform_blocked',
  SEARCH_LIST_UNSTABLE: 'search_list_unstable',
  LOGIN_REQUIRED: 'login_required',
  LOGIN_EXPIRED: 'login_expired',
  CONTENT_NOT_FOUND: 'content_not_found',
  HEARTBEAT_ONLY_STALL: 'heartbeat_only_stall',
  EXECUTOR_BUSY: 'executor_busy',
  STORAGE_WRITE_FAILED: 'storage_write_failed',
  DOWNLOAD_FAILED: 'download_failed',
  TASK_STOPPED_BY_USER: 'task_stopped_by_user',
  UNEXPECTED_INTERNAL_ERROR: 'unexpected_internal_error',
};

export const REMOTE_TARGET_PAGE_TYPE = {
  SEARCH: 'search',
  PROFILE: 'profile',
  DETAIL: 'detail',
  UNKNOWN: 'unknown',
};

export const WORKBENCH_DISPATCH_TARGET = {
  BACKGROUND: 'background',
  CONTENT: 'content',
};

export const SUPPORTED_REMOTE_TASKS = {
  [REMOTE_TASK_TYPE.XHS_BATCH_NOTES]: {
    platform: 'xhs',
    targetPageTypes: [REMOTE_TARGET_PAGE_TYPE.SEARCH, REMOTE_TARGET_PAGE_TYPE.PROFILE, REMOTE_TARGET_PAGE_TYPE.DETAIL],
    dispatchTarget: WORKBENCH_DISPATCH_TARGET.BACKGROUND,
    startAction: MSG.START_BATCH_NOTES,
    controlActions: {
      [REMOTE_TASK_CONTROL_ACTION.PAUSE]: MSG.PAUSE_BATCH_NOTES,
      [REMOTE_TASK_CONTROL_ACTION.RESUME]: MSG.RESUME_BATCH_NOTES,
      [REMOTE_TASK_CONTROL_ACTION.STOP]: MSG.STOP_BATCH_NOTES,
      [REMOTE_TASK_CONTROL_ACTION.DELETE]: MSG.STOP_BATCH_NOTES,
    },
    capabilityKey: 'canBatchNotes',
  },
  [REMOTE_TASK_TYPE.XHS_BATCH_COMMENTS]: {
    platform: 'xhs',
    targetPageTypes: [REMOTE_TARGET_PAGE_TYPE.SEARCH, REMOTE_TARGET_PAGE_TYPE.PROFILE, REMOTE_TARGET_PAGE_TYPE.DETAIL],
    dispatchTarget: WORKBENCH_DISPATCH_TARGET.BACKGROUND,
    startAction: MSG.START_BATCH_COMMENTS,
    controlActions: {
      [REMOTE_TASK_CONTROL_ACTION.PAUSE]: MSG.PAUSE_BATCH_COMMENTS,
      [REMOTE_TASK_CONTROL_ACTION.RESUME]: MSG.RESUME_BATCH_COMMENTS,
      [REMOTE_TASK_CONTROL_ACTION.STOP]: MSG.STOP_BATCH_COMMENTS,
      [REMOTE_TASK_CONTROL_ACTION.DELETE]: MSG.STOP_BATCH_COMMENTS,
    },
    capabilityKey: 'canBatchComments',
  },
  [REMOTE_TASK_TYPE.XHS_COLLECT_AUTHOR]: {
    platform: 'xhs',
    targetPageTypes: [REMOTE_TARGET_PAGE_TYPE.PROFILE],
    dispatchTarget: WORKBENCH_DISPATCH_TARGET.CONTENT,
    startAction: MSG.COLLECT_AUTHOR,
    controlActions: {
      [REMOTE_TASK_CONTROL_ACTION.PAUSE]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.RESUME]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.STOP]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.DELETE]: MSG.WORKBENCH_TASK_CONTROL,
    },
    capabilityKey: 'canCollectAuthor',
  },
  [REMOTE_TASK_TYPE.XHS_AUTHOR_NOTE_LINKS]: {
    platform: 'xhs',
    targetPageTypes: [REMOTE_TARGET_PAGE_TYPE.PROFILE],
    dispatchTarget: WORKBENCH_DISPATCH_TARGET.CONTENT,
    startAction: MSG.DISCOVER_AUTHOR_NOTE_LINKS,
    controlActions: {
      [REMOTE_TASK_CONTROL_ACTION.PAUSE]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.RESUME]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.STOP]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.DELETE]: MSG.WORKBENCH_TASK_CONTROL,
    },
    capabilityKey: 'canBatchNotes',
  },
  [REMOTE_TASK_TYPE.XHS_LIST_SCAN]: {
    platform: 'xhs',
    targetPageTypes: [REMOTE_TARGET_PAGE_TYPE.SEARCH, REMOTE_TARGET_PAGE_TYPE.PROFILE],
    dispatchTarget: WORKBENCH_DISPATCH_TARGET.BACKGROUND,
    startAction: MSG.START_BATCH_NOTES,
    controlActions: {
      [REMOTE_TASK_CONTROL_ACTION.PAUSE]: MSG.PAUSE_BATCH_NOTES,
      [REMOTE_TASK_CONTROL_ACTION.RESUME]: MSG.RESUME_BATCH_NOTES,
      [REMOTE_TASK_CONTROL_ACTION.STOP]: MSG.STOP_BATCH_NOTES,
      [REMOTE_TASK_CONTROL_ACTION.DELETE]: MSG.STOP_BATCH_NOTES,
    },
    capabilityKey: 'canBatchNotes',
  },
  [REMOTE_TASK_TYPE.XHS_NOTE_FULL]: {
    platform: 'xhs',
    targetPageTypes: [REMOTE_TARGET_PAGE_TYPE.DETAIL],
    dispatchTarget: WORKBENCH_DISPATCH_TARGET.BACKGROUND,
    startAction: MSG.START_BATCH_NOTES,
    controlActions: {
      [REMOTE_TASK_CONTROL_ACTION.PAUSE]: MSG.PAUSE_BATCH_NOTES,
      [REMOTE_TASK_CONTROL_ACTION.RESUME]: MSG.RESUME_BATCH_NOTES,
      [REMOTE_TASK_CONTROL_ACTION.STOP]: MSG.STOP_BATCH_NOTES,
      [REMOTE_TASK_CONTROL_ACTION.DELETE]: MSG.STOP_BATCH_NOTES,
    },
    capabilityKey: 'canBatchNotes',
  },
  [REMOTE_TASK_TYPE.XHS_COMMENT_SCAN]: {
    platform: 'xhs',
    targetPageTypes: [REMOTE_TARGET_PAGE_TYPE.DETAIL],
    dispatchTarget: WORKBENCH_DISPATCH_TARGET.BACKGROUND,
    startAction: MSG.START_BATCH_COMMENTS,
    controlActions: {
      [REMOTE_TASK_CONTROL_ACTION.PAUSE]: MSG.PAUSE_BATCH_COMMENTS,
      [REMOTE_TASK_CONTROL_ACTION.RESUME]: MSG.RESUME_BATCH_COMMENTS,
      [REMOTE_TASK_CONTROL_ACTION.STOP]: MSG.STOP_BATCH_COMMENTS,
      [REMOTE_TASK_CONTROL_ACTION.DELETE]: MSG.STOP_BATCH_COMMENTS,
    },
    capabilityKey: 'canBatchComments',
  },
  [REMOTE_TASK_TYPE.XHS_AUTHOR_PROFILE]: {
    platform: 'xhs',
    targetPageTypes: [REMOTE_TARGET_PAGE_TYPE.PROFILE],
    dispatchTarget: WORKBENCH_DISPATCH_TARGET.CONTENT,
    startAction: MSG.COLLECT_AUTHOR,
    controlActions: {
      [REMOTE_TASK_CONTROL_ACTION.PAUSE]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.RESUME]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.STOP]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.DELETE]: MSG.WORKBENCH_TASK_CONTROL,
    },
    capabilityKey: 'canCollectAuthor',
  },
  [REMOTE_TASK_TYPE.XHS_AUTHOR_LINKS]: {
    platform: 'xhs',
    targetPageTypes: [REMOTE_TARGET_PAGE_TYPE.PROFILE],
    dispatchTarget: WORKBENCH_DISPATCH_TARGET.CONTENT,
    startAction: MSG.DISCOVER_AUTHOR_NOTE_LINKS,
    controlActions: {
      [REMOTE_TASK_CONTROL_ACTION.PAUSE]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.RESUME]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.STOP]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.DELETE]: MSG.WORKBENCH_TASK_CONTROL,
    },
    capabilityKey: 'canBatchNotes',
  },
  [REMOTE_TASK_TYPE.DOUYIN_BATCH_NOTES]: {
    platform: 'douyin',
    targetPageTypes: [REMOTE_TARGET_PAGE_TYPE.SEARCH, REMOTE_TARGET_PAGE_TYPE.PROFILE, REMOTE_TARGET_PAGE_TYPE.DETAIL],
    dispatchTarget: WORKBENCH_DISPATCH_TARGET.BACKGROUND,
    startAction: MSG.START_BATCH_NOTES,
    controlActions: {
      [REMOTE_TASK_CONTROL_ACTION.PAUSE]: MSG.PAUSE_BATCH_NOTES,
      [REMOTE_TASK_CONTROL_ACTION.RESUME]: MSG.RESUME_BATCH_NOTES,
      [REMOTE_TASK_CONTROL_ACTION.STOP]: MSG.STOP_BATCH_NOTES,
      [REMOTE_TASK_CONTROL_ACTION.DELETE]: MSG.STOP_BATCH_NOTES,
    },
    capabilityKey: 'canBatchNotes',
  },
  [REMOTE_TASK_TYPE.DOUYIN_BATCH_COMMENTS]: {
    platform: 'douyin',
    targetPageTypes: [REMOTE_TARGET_PAGE_TYPE.SEARCH, REMOTE_TARGET_PAGE_TYPE.PROFILE],
    dispatchTarget: WORKBENCH_DISPATCH_TARGET.BACKGROUND,
    startAction: MSG.START_BATCH_COMMENTS,
    controlActions: {
      [REMOTE_TASK_CONTROL_ACTION.PAUSE]: MSG.PAUSE_BATCH_COMMENTS,
      [REMOTE_TASK_CONTROL_ACTION.RESUME]: MSG.RESUME_BATCH_COMMENTS,
      [REMOTE_TASK_CONTROL_ACTION.STOP]: MSG.STOP_BATCH_COMMENTS,
      [REMOTE_TASK_CONTROL_ACTION.DELETE]: MSG.STOP_BATCH_COMMENTS,
    },
    capabilityKey: 'canBatchComments',
  },
  [REMOTE_TASK_TYPE.DOUYIN_COLLECT_AUTHOR]: {
    platform: 'douyin',
    targetPageTypes: [REMOTE_TARGET_PAGE_TYPE.PROFILE],
    dispatchTarget: WORKBENCH_DISPATCH_TARGET.CONTENT,
    startAction: MSG.COLLECT_AUTHOR,
    controlActions: {
      [REMOTE_TASK_CONTROL_ACTION.PAUSE]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.RESUME]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.STOP]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.DELETE]: MSG.WORKBENCH_TASK_CONTROL,
    },
    capabilityKey: 'canCollectAuthor',
  },
  [REMOTE_TASK_TYPE.DOUYIN_SINGLE_COMMENTS]: {
    platform: 'douyin',
    targetPageTypes: [REMOTE_TARGET_PAGE_TYPE.DETAIL],
    dispatchTarget: WORKBENCH_DISPATCH_TARGET.CONTENT,
    startAction: MSG.COLLECT_SINGLE_COMMENT,
    controlActions: {
      [REMOTE_TASK_CONTROL_ACTION.PAUSE]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.RESUME]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.STOP]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.DELETE]: MSG.WORKBENCH_TASK_CONTROL,
    },
    capabilityKey: 'canCollectComments',
  },
  [REMOTE_TASK_TYPE.DOUYIN_COMMENT_IMAGE_DOWNLOAD]: {
    platform: 'douyin',
    targetPageTypes: [REMOTE_TARGET_PAGE_TYPE.DETAIL],
    dispatchTarget: WORKBENCH_DISPATCH_TARGET.CONTENT,
    startAction: MSG.DOWNLOAD_CURRENT_COMMENT_IMAGES,
    controlActions: {
      [REMOTE_TASK_CONTROL_ACTION.PAUSE]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.RESUME]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.STOP]: MSG.WORKBENCH_TASK_CONTROL,
      [REMOTE_TASK_CONTROL_ACTION.DELETE]: MSG.WORKBENCH_TASK_CONTROL,
    },
    capabilityKey: 'canDownloadCommentImages',
  },
};

export function getSupportedRemoteTask(taskType = '') {
  const normalizedTaskType = String(taskType || '').trim();
  return SUPPORTED_REMOTE_TASKS[normalizedTaskType] || null;
}
