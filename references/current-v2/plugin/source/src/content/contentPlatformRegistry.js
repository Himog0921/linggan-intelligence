import { isDouyinHostname } from './platformHostMatcher.js';

export const CONTENT_PLATFORM = Object.freeze({
  XHS: 'xhs',
  DOUYIN: 'douyin',
});

export const CONTENT_PLATFORM_REGISTRY = Object.freeze([
  {
    platform: CONTENT_PLATFORM.DOUYIN,
    matchesHostname: isDouyinHostname,
  },
]);

export function resolveContentPlatform(hostname = '') {
  const normalizedHostname = String(hostname || '').trim().toLowerCase();

  const matchedPlatform = CONTENT_PLATFORM_REGISTRY.find(({ matchesHostname }) => (
    matchesHostname(normalizedHostname)
  ));

  return matchedPlatform?.platform || CONTENT_PLATFORM.XHS;
}
