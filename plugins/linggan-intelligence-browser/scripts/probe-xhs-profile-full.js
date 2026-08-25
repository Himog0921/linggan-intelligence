/**
 * 小红书博主页全量探查 v1.0（2026-04-18）
 *
 * 在任意小红书博主主页（/user/profile/xxx）DevTools Console 执行
 *
 * 覆盖：
 * 1. __INITIAL_STATE__ 用户数据 + Vue ref 拆包验证
 * 2. DOM 选择器全量验证
 * 3. 粉丝/关注/互动数据来源对比
 * 4. 批量发现选择器验证（笔记卡片列表）
 */
(function probeXhsProfileFull() {
  const PROBE_VERSION = '1.0';

  function short(v, max = 160) {
    const t = String(v ?? '');
    return t.length > max ? t.slice(0, max) + '...' : t;
  }

  function selectorCheck(sel) {
    const el = document.querySelector(sel);
    if (!el) return { found: false, text: '', tag: '', classes: '' };
    return {
      found: true,
      text: short((el.textContent || '').trim()),
      tag: el.tagName,
      classes: short(el.className || ''),
    };
  }

  function selectorCountCheck(sel) {
    const els = document.querySelectorAll(sel);
    return {
      count: els.length,
      samples: Array.from(els).slice(0, 3).map(el => ({
        tag: el.tagName,
        text: short((el.textContent || '').trim(), 60),
        href: el.getAttribute('href') ? short(el.getAttribute('href'), 80) : undefined,
      })),
    };
  }

  // ============ 1. __INITIAL_STATE__ 用户数据 ============
  const state = window.__INITIAL_STATE__ || null;
  const userModule = state?.user || null;

  // Vue ref 拆包检测
  function unwrapRef(obj) {
    if (!obj) return { raw: null, isRef: false };
    if (obj._rawValue !== undefined) return { raw: obj._rawValue, isRef: true };
    if (obj.__v_isRef) return { raw: obj.value, isRef: true };
    return { raw: obj, isRef: false };
  }

  const userPageDataRaw = userModule?.userPageData || null;
  const userPageData = unwrapRef(userPageDataRaw);
  const userInfoRaw = userModule?.userInfo || null;
  const userInfo = unwrapRef(userInfoRaw);

  const userData = userPageData.raw || {};
  const basicInfo = userData.basicInfo || {};
  const interactions = userData.interactions || [];
  const extraInfo = userData.extraInfo || {};
  const tags = userData.tags || [];

  const stateCheck = {
    hasState: Boolean(state),
    stateTopKeys: state ? Object.keys(state).slice(0, 20) : [],
    hasUserModule: Boolean(userModule),
    userModuleKeys: userModule ? Object.keys(userModule).slice(0, 10) : [],
    userPageData: {
      exists: Boolean(userPageDataRaw),
      isVueRef: userPageData.isRef,
      hasRawValue: userPageDataRaw?._rawValue !== undefined,
      unwrappedKeys: userData ? Object.keys(userData).slice(0, 15) : [],
    },
    userInfo: {
      exists: Boolean(userInfoRaw),
      isVueRef: userInfo.isRef,
      hasRawValue: userInfoRaw?._rawValue !== undefined,
    },
  };

  // ============ 2. 结构化数据字段 ============
  const stateFields = {
    userId: short(basicInfo.userId || userData.userId || ''),
    redId: short(basicInfo.redId || userData.redId || ''),
    nickname: short(basicInfo.nickname || userData.nickname || ''),
    avatar: short(basicInfo.imageb || basicInfo.images || userData.imageb || '', 100),
    desc: short(basicInfo.desc || userData.desc || '', 120),
    ipLocation: short(basicInfo.ipLocation || ''),
    gender: basicInfo.gender,
    interactions: interactions.length > 0
      ? interactions.map(item => ({
          name: item.name,
          count: item.count,
          type: item.type,
        }))
      : 'NOT_FOUND',
    tags: tags.length > 0 ? tags.map(t => short(t.name || t.tagType || JSON.stringify(t), 40)) : 'NOT_FOUND',
    accountStatus: extraInfo.blockType,
    followedByMe: extraInfo.fstatus,
  };

  // ============ 3. DOM 选择器验证 ============
  const domSelectors = {
    userName: selectorCheck('div.user-name'),
    userRedId: selectorCheck('span.user-redId'),
    userIP: selectorCheck('span.user-IP'),
    userDesc: selectorCheck('div.user-desc'),
    userImage: (() => {
      const img = document.querySelector('img.user-image');
      return img
        ? { found: true, src: short(img.src || img.getAttribute('src') || '', 120) }
        : { found: false, src: '' };
    })(),
    showsLabels: selectorCountCheck('span.shows'),
  };

  // DOM vs State 对比
  const crossCheck = {
    name_match: domSelectors.userName.found && stateFields.nickname
      ? domSelectors.userName.text === stateFields.nickname
      : null,
    redId_dom_raw: domSelectors.userRedId.text,
    redId_state: stateFields.redId,
    ip_dom_raw: domSelectors.userIP.text,
    ip_state: stateFields.ipLocation,
  };

  // ============ 4. 批量发现选择器验证 ============
  const batchSelectors = {
    feedsContainer: selectorCheck('.feeds-container'),
    userPostedFeeds: selectorCheck('#userPostedFeeds'),
    sectionCards: selectorCountCheck('section'),
    cardCoverLinks: selectorCountCheck('section a.cover'),
    cardFooterTitle: selectorCountCheck('section .footer span'),
    cardLikeCount: selectorCountCheck('section .like-wrapper .count'),
    cardPlayIcon: selectorCountCheck('section .play-icon'),
  };

  // 提取前3个卡片的 noteId
  const cardSamples = [];
  document.querySelectorAll('section a.cover').forEach((a, i) => {
    if (i >= 3) return;
    const href = a.getAttribute('href') || '';
    const noteIdMatch = href.match(/\/(?:explore|discovery\/item)\/([a-f0-9]+)/);
    cardSamples.push({
      index: i,
      href: short(href, 100),
      noteId: noteIdMatch?.[1] || 'PARSE_FAILED',
      hasImg: Boolean(a.querySelector('img')),
    });
  });

  // ============ 诊断 ============
  const diagnosis = [];
  if (!state) diagnosis.push('CRITICAL: window.__INITIAL_STATE__ 不存在');
  if (!userModule) diagnosis.push('CRITICAL: __INITIAL_STATE__.user 不存在');
  if (userPageDataRaw && !userPageData.isRef) diagnosis.push('INFO: userPageData 非 Vue ref（可能框架已变化）');
  if (userPageData.isRef && !userPageDataRaw._rawValue) diagnosis.push('CRITICAL: userPageData 是 Vue ref 但 _rawValue 为空');
  if (!stateFields.userId) diagnosis.push('WARNING: 未从 State 中获取到 userId');
  if (stateFields.interactions === 'NOT_FOUND') diagnosis.push('WARNING: 未找到粉丝/关注/互动数据（interactions）');
  if (!domSelectors.userName.found) diagnosis.push('SELECTOR_BROKEN: div.user-name 未找到');
  if (!domSelectors.userRedId.found) diagnosis.push('SELECTOR_BROKEN: span.user-redId 未找到');
  if (!domSelectors.userIP.found) diagnosis.push('INFO: span.user-IP 未找到（某些页面不显示IP）');
  if (batchSelectors.cardCoverLinks.count === 0) diagnosis.push('WARNING: 未发现笔记卡片（section a.cover），批量发现可能失效');
  if (crossCheck.name_match === false) diagnosis.push('MISMATCH: DOM 用户名与 State 用户名不一致');

  const output = {
    probeVersion: PROBE_VERSION,
    probeType: 'xhs-profile-full',
    time: new Date().toISOString(),
    url: location.href,
    stateCheck,
    stateFields,
    domSelectors,
    crossCheck,
    batchSelectors,
    cardSamples,
    diagnosis,
  };

  window.__XHS_PROFILE_PROBE__ = output;
  console.group('[probe-xhs-profile-full]');
  console.log('result:', output);
  console.log('json:', JSON.stringify(output, null, 2));
  console.groupEnd();
  return output;
})();
