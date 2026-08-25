const DOUYIN_HOST_PATTERN = /(^|\.)douyin\.com$/i;

export function isDouyinHostname(hostname = '') {
  return DOUYIN_HOST_PATTERN.test(String(hostname || ''));
}
