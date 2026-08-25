/**
 * 抖音博主页字段探针（在抖音博主页 DevTools Console 执行）
 *
 * 目标：
 * 1. 验证“当前主页博主”与“登录用户”是否串线
 * 2. 输出抖音号 / secUid / IP / 统计字段的候选来源
 * 3. 为重建博主采集器提供字段证据
 */
(function probeDouyinAuthorFields() {
  const MAX_SCAN_RESULTS = 140;

  function short(value, max = 180) {
    const text = String(value ?? '');
    return text.length > max ? `${text.slice(0, max)}...` : text;
  }

  function parseJsonCandidates(raw) {
    if (!raw) return null;
    const candidates = [String(raw)];
    try {
      const decoded = decodeURIComponent(String(raw));
      if (decoded !== raw) candidates.push(decoded);
      const twiceDecoded = decodeURIComponent(decoded);
      if (twiceDecoded !== decoded) candidates.push(twiceDecoded);
    } catch {
      // ignore
    }
    for (const item of candidates) {
      try {
        return JSON.parse(item);
      } catch {
        // try next
      }
    }
    return null;
  }

  function getRenderData() {
    const raw = document.getElementById('RENDER_DATA')?.innerHTML || '';
    return parseJsonCandidates(raw);
  }

  function getByPath(root, path) {
    if (!root || !path) return undefined;
    const segs = path.split('.');
    let cur = root;
    for (const seg of segs) {
      if (cur == null) return undefined;
      cur = cur[seg];
    }
    return cur;
  }

  function parseUrlUserId() {
    const pathname = location.pathname || '';
    const userMatch = pathname.match(/^\/user\/([^/?#]+)/);
    if (userMatch?.[1]) {
      try {
        return decodeURIComponent(userMatch[1]);
      } catch {
        return userMatch[1];
      }
    }
    const atMatch = pathname.match(/^\/@([^/?#]+)/);
    return atMatch?.[1] || '';
  }

  function collectText(selector) {
    const el = document.querySelector(selector);
    if (!el) return { found: false, text: '', html: '' };
    return {
      found: true,
      text: short((el.textContent || '').trim()),
      html: short((el.outerHTML || '').replace(/\s+/g, ' ')),
    };
  }

  function scanKeys(root, patterns, max = MAX_SCAN_RESULTS) {
    const out = [];
    const queue = [{ node: root, path: 'root', depth: 0 }];
    const visited = new Set();
    while (queue.length > 0 && out.length < max) {
      const { node, path, depth } = queue.shift();
      if (!node || typeof node !== 'object' || depth > 8) continue;
      if (visited.has(node)) continue;
      visited.add(node);
      const entries = Array.isArray(node) ? node.entries() : Object.entries(node);
      for (const [key, value] of entries) {
        const k = String(key);
        const p = `${path}.${k}`;
        const lower = k.toLowerCase();
        if (patterns.some((item) => lower.includes(item.toLowerCase()))) {
          const type = value == null ? 'nullish' : (Array.isArray(value) ? 'array' : typeof value);
          let preview = '';
          if (type === 'string' || type === 'number' || type === 'boolean') preview = short(value);
          if (type === 'array') preview = `len=${value.length}`;
          if (type === 'object') preview = `keys=${Object.keys(value || {}).slice(0, 8).join(',')}`;
          out.push({ path: p, key: k, type, preview });
          if (out.length >= max) break;
        }
        if (value && typeof value === 'object') queue.push({ node: value, path: p, depth: depth + 1 });
      }
    }
    return out;
  }

  function parseDouyinIdText(text = '') {
    const match = String(text || '').match(/抖音号[:：]\s*([A-Za-z0-9._-]{2,40})/);
    return match?.[1] || '';
  }

  function collectIpMarkerSamples(limit = 6) {
    const samples = [];
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    let node = walker.nextNode();
    while (node && samples.length < limit) {
      const text = String(node.textContent || '').trim();
      if (text && /IP属地/i.test(text)) {
        samples.push(short(text, 120));
      }
      node = walker.nextNode();
    }
    return samples;
  }

  const renderData = getRenderData();
  const initialState = window.__INITIAL_STATE__ || null;
  const renderUser = getByPath(renderData, 'app.user.info') || null;
  const initUser = getByPath(initialState, 'user.userInfo._rawValue') || getByPath(initialState, 'user.userInfo') || null;
  const urlUserId = parseUrlUserId();
  const renderSecUid = String(renderUser?.secUid || renderUser?.sec_uid || '').trim();
  const initSecUid = String(initUser?.secUid || initUser?.sec_uid || '').trim();

  const userDetail = collectText('[data-e2e="user-detail"]');
  const signature = collectText('[data-e2e="user-signature"]');
  const userIp = collectText('[data-e2e="user-ip"]');

  const output = {
    time: new Date().toISOString(),
    url: location.href,
    title: document.title,
    page: {
      pathname: location.pathname,
      urlUserId,
      hasRenderData: Boolean(renderData),
      hasInitialState: Boolean(initialState),
    },
    identity: {
      renderSecUid,
      initSecUid,
      matchedRenderUser: Boolean(urlUserId && renderSecUid && urlUserId === renderSecUid),
      matchedInitUser: Boolean(urlUserId && initSecUid && urlUserId === initSecUid),
      riskUsingRender: Boolean(renderUser && urlUserId && renderSecUid && urlUserId !== renderSecUid),
    },
    directFields: {
      nickname_render: short(renderUser?.nickname || ''),
      nickname_init: short(initUser?.nickname || ''),
      nickname_dom: userDetail.text,
      douyinId_render: short(renderUser?.uniqueId || renderUser?.unique_id || renderUser?.shortId || renderUser?.short_id || ''),
      douyinId_init: short(initUser?.uniqueId || initUser?.unique_id || initUser?.shortId || initUser?.short_id || ''),
      douyinId_dom: parseDouyinIdText(`${userDetail.text}\n${signature.text}`),
      ip_render: short(renderUser?.ipLocation || renderUser?.ip_location || ''),
      ip_init: short(initUser?.ipLocation || initUser?.ip_location || ''),
      ip_dom_userIp: userIp.text,
      ip_dom_samples: collectIpMarkerSamples(),
      stats_render: {
        fans: Number(renderUser?.mplatformFollowersCount || renderUser?.followerCount || 0),
        follows: Number(renderUser?.followingCount || 0),
        interactions: Number(renderUser?.totalFavorited || 0),
        awemeCount: Number(renderUser?.awemeCount || renderUser?.aweme_count || 0),
      },
      stats_dom_text: {
        follow: collectText('[data-e2e="user-info-follow"]').text,
        fans: collectText('[data-e2e="user-info-fans"]').text,
        likes: collectText('[data-e2e="user-info-like"]').text,
        postCount: collectText('[data-e2e="user-tab-count"]').text,
      },
    },
    selectorSnapshot: {
      userDetail,
      signature,
      avatar: {
        found: Boolean(document.querySelector('img[data-e2e="user-avatar"]')),
        src: short(document.querySelector('img[data-e2e="user-avatar"]')?.getAttribute('src') || '', 220),
      },
      userIp,
      badge: collectText('[data-e2e="badge-role-name"]'),
    },
    renderUserKeyScan: scanKeys(renderUser, [
      'uid', 'sec', 'unique', 'short', 'nick', 'ip', 'location',
      'follower', 'following', 'favorited', 'aweme', 'avatar', 'gender', 'desc',
    ]),
    initialUserKeyScan: scanKeys(initUser, [
      'uid', 'sec', 'unique', 'short', 'nick', 'ip', 'location',
      'follower', 'following', 'favorited', 'aweme', 'avatar', 'gender', 'desc',
    ], 90),
    diagnosis: [],
  };

  if (!urlUserId) {
    output.diagnosis.push('URL 中未解析到 userId（请确认当前在 /user/... 页面）');
  }
  if (output.identity.riskUsingRender) {
    output.diagnosis.push('RENDER_DATA 用户与当前主页 userId 不一致，存在“串成登录账号”风险');
  }
  if (!output.directFields.douyinId_render && !output.directFields.douyinId_init && !output.directFields.douyinId_dom) {
    output.diagnosis.push('未发现明确抖音号候选值');
  }
  if (!output.directFields.ip_render && !output.directFields.ip_init && !output.directFields.ip_dom_userIp) {
    output.diagnosis.push('未发现明确 IP 属地候选值');
  }

  window.__DY_AUTHOR_FIELD_PROBE__ = output;
  console.group('[probe-douyin-author-fields]');
  console.log('result:', output);
  console.log('json:', JSON.stringify(output, null, 2));
  console.groupEnd();
  return output;
})();
