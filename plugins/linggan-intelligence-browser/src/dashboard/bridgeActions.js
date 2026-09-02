// Dashboard actions travel only across the trusted Dashboard iframe → active page bridge.
// Keeping them outside the retired MSG catalogue prevents a cache-inspection affordance from
// accidentally reviving the legacy Workbench transport.
export const DASHBOARD_BRIDGE_ACTION = {
  SYNC_AUTHORS_TO_OBSERVATION_TARGETS: 'syncAuthorsToObservationTargets',
};
