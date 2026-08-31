import { BATCH_CONFIG } from './constants.js';

export function requireBatchTargetCount(value) {
  const count = Number(value);
  if (!Number.isInteger(count) || count < 1 || count > BATCH_CONFIG.maxPerSession) {
    throw new Error(`batch_target_count_must_be_1_to_${BATCH_CONFIG.maxPerSession}`);
  }
  return count;
}
