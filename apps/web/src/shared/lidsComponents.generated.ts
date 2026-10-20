// Generated from docs/design/lids/component-contracts.json by lids:sync.
export const ACTION_VARIANTS = ["primary","ink","outline","soft","danger","quiet","loading"] as const;
export type ActionVariant = typeof ACTION_VARIANTS[number];
export const FIND_TRIGGERS = ["submit","debounce_250"] as const;
export type FindTrigger = typeof FIND_TRIGGERS[number];
export const FIND_DEBOUNCE_MS = 250;
