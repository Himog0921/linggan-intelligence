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
};
