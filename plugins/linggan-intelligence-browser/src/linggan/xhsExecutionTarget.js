function text(value) {
  return String(value || '').trim();
}

function noteIdFromPath(pathname) {
  return pathname.match(/^\/(?:explore|discovery\/item|search_result)\/([^/?#]+)/i)?.[1]
    || pathname.match(/^\/user\/profile\/[^/?#]+\/([^/?#]+)/i)?.[1]
    || '';
}

/**
 * Convert a server-selected, source-observed signed URL into the one XHS detail shape that the
 * execution lane accepts. The token is short-lived execution context, not content identity.
 */
export function buildSignedXhsDetailExecutionUrl(contentExternalId, executionSourceUrl) {
  const expectedId = text(contentExternalId).replace(/^xhs_/i, '');
  if (!expectedId) return null;
  try {
    const source = new URL(text(executionSourceUrl));
    const host = source.hostname.replace(/^www\./i, '').toLowerCase();
    if (source.protocol !== 'https:' || host !== 'xiaohongshu.com' || source.username || source.password || source.port) return null;
    const observedId = decodeURIComponent(noteIdFromPath(source.pathname)).replace(/^xhs_/i, '');
    const token = text(source.searchParams.get('xsec_token'));
    if (observedId !== expectedId || !token) return null;

    const target = new URL(`https://www.xiaohongshu.com/discovery/item/${encodeURIComponent(expectedId)}`);
    target.searchParams.set('source', 'webshare');
    target.searchParams.set('xhsshare', 'pc_web');
    target.searchParams.set('xsec_token', token);
    target.searchParams.set('xsec_source', 'pc_share');
    return target.toString();
  } catch {
    return null;
  }
}
