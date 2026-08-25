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
  COLLECT_CURRENT_COMMENTS: 'lingganCollectCurrentComments',
  COLLECT_CURRENT_AUTHOR: 'lingganCollectCurrentAuthor',
  START_BATCH_CONTENT: 'lingganStartBatchContent',
  START_BATCH_COMMENTS: 'lingganStartBatchComments',
  ACQUIRE_COMMENT_MEDIA: 'lingganAcquireCommentMedia',
  PAUSE_ACTIVE_BATCH: 'lingganPauseActiveBatch',
  RESUME_ACTIVE_BATCH: 'lingganResumeActiveBatch',
  STOP_ACTIVE_BATCH: 'lingganStopActiveBatch',
};
