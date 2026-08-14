// 消息协议 action 常量
export const MSG = {
  // Content Script 操作
  COLLECT_SINGLE_NOTE: 'collectSingleNote',
  COLLECT_SINGLE_COMMENT: 'collectSingleComment',
  DOWNLOAD_CURRENT_COMMENT_IMAGES: 'downloadCurrentCommentImages',
  COLLECT_AUTHOR: 'collectAuthor',
  DISCOVER_AUTHOR_NOTE_LINKS: 'discoverAuthorNoteLinks',

  // 批量操作
  START_BATCH_NOTES: 'startBatchNotes',
  STOP_BATCH_NOTES: 'stopBatchNotes',
  PAUSE_BATCH_NOTES: 'pauseBatchNotes',
  RESUME_BATCH_NOTES: 'resumeBatchNotes',
  START_BATCH_COMMENTS: 'startBatchComments',
  STOP_BATCH_COMMENTS: 'stopBatchComments',
  PAUSE_BATCH_COMMENTS: 'pauseBatchComments',
  RESUME_BATCH_COMMENTS: 'resumeBatchComments',

  // Background 操作
  BLOCK_MEDIA: 'blockMedia',
  UNBLOCK_MEDIA: 'unblockMedia',
  DISPATCH_ESC: 'dispatchEscapeViaDebugger',
  DOWNLOAD_MEDIA_FILE: 'downloadMediaFile',
  FETCH_BINARY_AS_DATA_URL: 'fetchBinaryAsDataUrl',
  RELEASE_EXECUTION_ACCOUNT_LOCK: 'releaseExecutionAccountLock',

  // 数据操作
  GET_STATS: 'getStats',
  EXPORT_CSV: 'exportCsv',
  EXPORT_JSON: 'exportJson',
  RUN_DATA_MAINTENANCE: 'runDataMaintenance',
  TOGGLE_DASHBOARD: 'toggleDashboard',
  GET_ALL_NOTES: 'getAllNotes',
  GET_ALL_COMMENTS: 'getAllComments',
  GET_ALL_AUTHORS: 'getAllAuthors',
  GET_PAGE_CONTEXT: 'getPageContext',
  DELETE_NOTE: 'deleteNote',
  DELETE_COMMENT: 'deleteComment',
  DELETE_AUTHOR: 'deleteAuthor',
  CLEAR_ALL_NOTES: 'clearAllNotes',
  CLEAR_ALL_COMMENTS: 'clearAllComments',
  CLEAR_ALL_AUTHORS: 'clearAllAuthors',
  DOWNLOAD_NOTE_MEDIA: 'downloadNoteMedia',

  // 飞轮同步
  TEST_FLYWHEEL_CONNECTION: 'testFlywheelConnection',
  GET_FLYWHEEL_CONFIG: 'getFlywheelConfig',
  SAVE_FLYWHEEL_CONFIG: 'saveFlywheelConfig',

  // 工作台接入
  WORKBENCH_CAPABILITY_CHECK: 'workbenchCapabilityCheck',
  WORKBENCH_DISPATCH_TASK: 'workbenchDispatchTask',
  WORKBENCH_TASK_CONTROL: 'workbenchTaskControl',
  WORKBENCH_GET_RESULT_PACKAGE: 'workbenchGetResultPackage',
  WORKBENCH_LOCAL_CONTROL_EVENT: 'workbenchLocalControlEvent',
  WORKBENCH_RECORD_DELTA: 'workbenchRecordDelta',
  WORKBENCH_DELTA_FLUSH: 'workbenchDeltaFlush',
  SYNC_TO_WORKBENCH: 'syncToWorkbench',
  AUTHORIZE_PLUGIN_ACCESS: 'authorizePluginAccess',
  REQUEST_PLUGIN_AUTHORIZATION: 'requestPluginAuthorization',
  CLAIM_PLUGIN_AUTHORIZATION_REQUEST: 'claimPluginAuthorizationRequest',
  CLEAR_PLUGIN_AUTHORIZATION: 'clearPluginAuthorization',
  GET_EXECUTION_STATION_STATUS: 'getExecutionStationStatus',
  EXPORT_OUTBOX_RECOVERY: 'exportOutboxRecovery',
  REGISTER_EXECUTION_STATION: 'registerExecutionStation',
  SEND_EXECUTION_STATION_HEARTBEAT: 'sendExecutionStationHeartbeat',

  // 账号管理
  GET_ACCOUNTS: 'getAccounts',
  ADD_ACCOUNT: 'addAccount',
  REMOVE_ACCOUNT: 'removeAccount',
  UPDATE_ACCOUNT: 'updateAccount',

  // Cookie 管理
  GET_PLATFORM_COOKIES: 'getPlatformCookies',
  GET_STORED_PLATFORM_COOKIES: 'getStoredPlatformCookies',

  // 进度与状态
  PROGRESS: 'progress',
  COLLECT_DONE: 'collectDone',
  ERROR: 'error',
};

export const TASK_STATE = {
  IDLE: 'idle',
  RUNNING: 'running',
  PAUSED: 'paused',
  STOPPING: 'stopping',
  DONE: 'done',
  ERROR: 'error',
};

// 采集模式
export const COLLECT_MODE = {
  SEARCH: 'search',
  PROFILE: 'profile',
  DETAIL: 'detail',
  FAVORITE: 'favorite',
};

export const COMMENT_DEPTH_MODE = {
  TWO_LEVEL: 'twoLevel',
  ALL_REPLIES: 'allReplies',
};

// 页面类型
export const PAGE_TYPE = {
  NOTE_DETAIL: 'noteDetail',
  SEARCH: 'search',
  PROFILE: 'profile',
  EXPLORE: 'explore',
  UNKNOWN: 'unknown',
};

// 批量采集配置
export const BATCH_CONFIG = {
  maxPerSession: 50,
  intervalMin: 1200,
  intervalMax: 2800,
  iframeTimeout: 15000,
  scrollStepMin: 100,
  scrollStepMax: 300,
  scrollIntervalMin: 200,
  scrollIntervalMax: 500,
  maxScrollRetries: 10,
  maxSubComments: 200,
};
