/**
 * 小红书作者主页探针 v2。
 *
 * 只输出资料与作品列表的字段存在性、已加载数量及停止线索；不输出作者身份、简介、链接或作品原文。
 */
(function probeXhsProfileV2() {
  const state = window.__INITIAL_STATE__ || null;
  const rawProfile = state?.user?.userPageData || null;
  const profile = rawProfile?._rawValue || rawProfile?.value || rawProfile || null;
  const basicInfo = profile?.basicInfo || profile || null;
  const cards = [...document.querySelectorAll('section')].filter((card) => card.querySelector('a.cover, a[href*="/explore/"], a[href*="/discovery/item/"]'));
  const stateInteractions = Array.isArray(profile?.interactions) ? profile.interactions : [];

  const output = {
    probeVersion: '2.0',
    probeType: 'xhs-profile',
    sanitized: true,
    profile: {
      embeddedStatePresent: Boolean(state),
      profileStatePresent: Boolean(rawProfile),
      fieldPresence: {
        nickname: Boolean(basicInfo?.nickname) || Boolean(document.querySelector('.user-name, [class*="user-name"]')),
        description: Boolean(basicInfo?.desc) || Boolean(document.querySelector('.user-desc, [class*="user-desc"]')),
        avatar: Boolean(basicInfo?.imageb || basicInfo?.images) || Boolean(document.querySelector('img.user-image, [class*="avatar"] img')),
        publicStats: stateInteractions.length > 0 || document.querySelectorAll('span.shows, [class*="interact"], [class*="stat"]').length > 0,
        tags: Array.isArray(profile?.tags) ? profile.tags.length > 0 : document.querySelectorAll('[class*="tag"]').length > 0,
      },
    },
    works: {
      loadedCardCount: cards.length,
      visibleCardCount: cards.filter((card) => card.getClientRects().length > 0).length,
      cardFieldPresence: {
        cover: cards.some((card) => Boolean(card.querySelector('img, video'))),
        titleOrSummary: cards.some((card) => Boolean(card.querySelector('.footer span, [class*="title"], [class*="desc"]'))),
        interaction: cards.some((card) => Boolean(card.querySelector('.like-wrapper, [class*="like"], [class*="interact"]'))),
      },
      hasLoadMoreControl: Boolean(document.querySelector('[class*="more"], [class*="load"]')),
      stopReason: 'not_observed_by_passive_probe',
    },
    limitations: [
      '不输出作者资料、作品标题、作品链接或统计数值。',
      '不滚动或展开作品列表；当前数量只是已加载页面状态。',
    ],
  };

  window.__XHS_PROFILE_PROBE_V2__ = output;
  console.info('[probe-xhs-profile-v2]', output);
  return output;
})();
