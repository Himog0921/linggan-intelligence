function normalizeString(value = '') {
  return String(value || '').trim();
}

function stripKnownContentPrefix(value = '') {
  return normalizeString(value).replace(/^(?:xhs_|dy_)/i, '');
}

function normalizeIdentity(value = '', { stripContentPrefix = false } = {}) {
  const normalized = stripContentPrefix ? stripKnownContentPrefix(value) : normalizeString(value);
  try {
    return decodeURIComponent(normalized).trim().toLowerCase();
  } catch {
    return normalized.trim().toLowerCase();
  }
}

function parseUrl(value = '', baseUrl = 'https://example.com') {
  const raw = normalizeString(value);
  if (!raw) return null;
  try {
    return new URL(raw, baseUrl);
  } catch {
    return null;
  }
}

function inferPlatformFromHost(hostname = '') {
  const host = normalizeString(hostname).toLowerCase();
  if (host.includes('xiaohongshu.com') || host.includes('xhslink.com')) return 'xhs';
  if (host.includes('douyin.com')) return 'douyin';
  return '';
}

function pathSegmentsFromUrl(url) {
  return normalizeString(url?.pathname)
    .split('/')
    .map((segment) => normalizeString(segment))
    .filter(Boolean);
}

function extractXhsProfileIdentity(segments = []) {
  const userProfileIndex = segments.findIndex((segment, index) => (
    segment.toLowerCase() === 'profile'
    && segments[index - 1]?.toLowerCase() === 'user'
  ));
  if (userProfileIndex >= 0) return normalizeIdentity(segments[userProfileIndex + 1]);

  const profileIndex = segments.findIndex((segment) => segment.toLowerCase() === 'profile');
  if (profileIndex >= 0) return normalizeIdentity(segments[profileIndex + 1]);

  const userIndex = segments.findIndex((segment) => segment.toLowerCase() === 'user');
  return userIndex >= 0 ? normalizeIdentity(segments[userIndex + 1]) : '';
}

function extractXhsContentIdentity(segments = []) {
  const directIndex = segments.findIndex((segment, index) => {
    const normalized = segment.toLowerCase();
    return normalized === 'explore'
      || (normalized === 'item' && segments[index - 1]?.toLowerCase() === 'discovery');
  });
  if (directIndex >= 0) return normalizeIdentity(segments[directIndex + 1], { stripContentPrefix: true });

  const userProfileIndex = segments.findIndex((segment, index) => (
    segment.toLowerCase() === 'profile'
    && segments[index - 1]?.toLowerCase() === 'user'
  ));
  return userProfileIndex >= 0
    ? normalizeIdentity(segments[userProfileIndex + 2], { stripContentPrefix: true })
    : '';
}

function extractDouyinProfileIdentity(segments = []) {
  const userIndex = segments.findIndex((segment) => segment.toLowerCase() === 'user');
  if (userIndex >= 0) return normalizeIdentity(segments[userIndex + 1]);
  const atSegment = segments.find((segment) => segment.startsWith('@'));
  return atSegment ? normalizeIdentity(atSegment.slice(1)) : '';
}

function extractDouyinContentIdentity(segments = []) {
  const directIndex = segments.findIndex((segment) => (
    segment.toLowerCase() === 'video'
    || segment.toLowerCase() === 'note'
  ));
  return directIndex >= 0 ? normalizeIdentity(segments[directIndex + 1], { stripContentPrefix: true }) : '';
}

export function parseTargetIdentity(value = '', { baseUrl = 'https://example.com' } = {}) {
  const url = parseUrl(value, baseUrl);
  if (!url) {
    return { platform: '', profileId: '', contentId: '' };
  }
  const platform = inferPlatformFromHost(url.hostname);
  const segments = pathSegmentsFromUrl(url);

  if (platform === 'xhs') {
    return {
      platform,
      profileId: extractXhsProfileIdentity(segments),
      contentId: extractXhsContentIdentity(segments),
    };
  }

  if (platform === 'douyin') {
    return {
      platform,
      profileId: extractDouyinProfileIdentity(segments),
      contentId: extractDouyinContentIdentity(segments),
    };
  }

  return { platform, profileId: '', contentId: '' };
}

export function extractProfileIdentityFromUrl(value = '', options = {}) {
  return parseTargetIdentity(value, options).profileId;
}

export function extractContentIdentityFromUrl(value = '', options = {}) {
  return parseTargetIdentity(value, options).contentId;
}

export function sameTargetIdentity(kind = '', left = '', right = '', options = {}) {
  const normalizedKind = normalizeString(kind).toLowerCase();
  const leftIdentity = parseTargetIdentity(left, options);
  const rightIdentity = parseTargetIdentity(right, options);
  if (normalizedKind === 'profile') {
    return Boolean(leftIdentity.profileId && leftIdentity.profileId === rightIdentity.profileId);
  }
  if (normalizedKind === 'detail' || normalizedKind === 'content') {
    return Boolean(leftIdentity.contentId && leftIdentity.contentId === rightIdentity.contentId);
  }
  return false;
}
