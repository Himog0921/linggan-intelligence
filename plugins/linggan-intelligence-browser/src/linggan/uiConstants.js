// Narrow UI constants used by the Linggan-owned active runtime.
// The rehomed legacy constants remain available to dormant collector source,
// but the active browser shell must not import the old workbench protocol.
export const TASK_STATE = {
  IDLE: 'idle',
  RUNNING: 'running',
  PAUSED: 'paused',
  STOPPING: 'stopping',
  DONE: 'done',
  ERROR: 'error',
};

export const PAGE_TYPE = {
  NOTE_DETAIL: 'noteDetail',
  SEARCH: 'search',
  PROFILE: 'profile',
  EXPLORE: 'explore',
  UNKNOWN: 'unknown',
};

export const COMMENT_DEPTH_MODE = {
  TWO_LEVEL: 'twoLevel',
  ALL_REPLIES: 'allReplies',
};
