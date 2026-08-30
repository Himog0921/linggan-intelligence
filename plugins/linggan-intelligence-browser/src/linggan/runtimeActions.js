// These values intentionally match the existing Popup and Dashboard UI
// protocol. Keeping this small map separate prevents the active Linggan
// runtime from loading the retired workbench action catalogue.
export const LINGGAN_RUNTIME_ACTION = {
  GET_STATS: 'getStats',
  GET_PAGE_CONTEXT: 'getPageContext',
  TOGGLE_DASHBOARD: 'toggleDashboard',
  GET_FLYWHEEL_CONFIG: 'getFlywheelConfig',
  SAVE_FLYWHEEL_CONFIG: 'saveFlywheelConfig',
  TEST_FLYWHEEL_CONNECTION: 'testFlywheelConnection',
  GET_EXECUTION_STATION_STATUS: 'getExecutionStationStatus',
  CREATE_MANUAL_TASK: 'lingganCreateManualTask',
  GET_PRODUCER_INSTANCE: 'lingganGetProducerInstance',
  SUBMIT_DISCOVERY_PACKAGE: 'lingganSubmitDiscoveryPackage',
  SUBMIT_CAPTURE_PACKAGE: 'lingganSubmitCapturePackage',
  SUBMIT_MEDIA_SLOTS: 'lingganSubmitMediaSlots',
  CREATE_SCHEDULED_TASK: 'lingganCreateScheduledTask',
  FLUSH_LOCAL_OUTBOX: 'lingganFlushLocalOutbox',
  // Popup commands are intentionally separate from the retired MSG.COLLECT_* catalogue.
  // They only reach the active page runtime, which owns the eventual package/receipt boundary.
  COLLECT_CURRENT_CONTENT: 'lingganCollectCurrentContent',
  // 服务端固定作品深化的单页全量读取。它只预取同一 Work Order 已批准的 lane；
  // 每个 lane 仍等待自己的 TaskSpec 后才形成 Package/Receipt。
  COLLECT_NOTE_FULL: 'lingganCollectNoteFull',
  COLLECT_CURRENT_COMMENTS: 'lingganCollectCurrentComments',
  COLLECT_CURRENT_AUTHOR: 'lingganCollectCurrentAuthor',
  // 表层发现面：作者页的作品清单。它只读页面已可见的卡片，**不打开任何详情**。
  DISCOVER_SURFACE: 'lingganDiscoverSurface',
  // 领取并执行服务端派下来的一个任务。
  RUN_DISPATCHED_TASK: 'lingganRunDispatchedTask',
  START_BATCH_CONTENT: 'lingganStartBatchContent',
  START_BATCH_COMMENTS: 'lingganStartBatchComments',
  ACQUIRE_COMMENT_MEDIA: 'lingganAcquireCommentMedia',
  PAUSE_ACTIVE_BATCH: 'lingganPauseActiveBatch',
  RESUME_ACTIVE_BATCH: 'lingganResumeActiveBatch',
  STOP_ACTIVE_BATCH: 'lingganStopActiveBatch',
};
