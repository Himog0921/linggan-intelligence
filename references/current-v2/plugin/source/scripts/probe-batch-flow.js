/**
 * 批量采集流程探查脚本（在小红书页面 DevTools Console 运行）
 *
 * 目的：
 * 1. 验证笔记卡片选择器是否稳定
 * 2. 验证点击卡片后是路由跳转还是弹窗模式
 * 3. 输出可用关闭按钮候选
 */
(function probeBatchFlow() {
  const result = {
    url: location.href,
    time: new Date().toISOString(),
    cardCount: 0,
    cardSelectors: {},
    openMode: 'unknown',
    closeSelectors: [],
  };

  const cardRoots = ['.feeds-container', '#userPostedFeeds'];
  cardRoots.forEach((rootSel) => {
    const sections = document.querySelectorAll(`${rootSel} section`);
    result.cardSelectors[rootSel] = sections.length;
    result.cardCount += sections.length;
  });

  const possibleClose = [
    '.close-circle .close.close-mask-dark',
    '.close-circle',
    '[class*="close-circle"]',
    '.note-detail-mask',
    '[class*="note-detail-mask"]',
    '[class*="back-btn"]',
    '.back-icon',
  ];

  possibleClose.forEach((sel) => {
    const el = document.querySelector(sel);
    if (el && el.offsetWidth > 0 && el.offsetHeight > 0) {
      result.closeSelectors.push(sel);
    }
  });

  if (/\/explore\/[a-z0-9]+/i.test(location.pathname) || /\/discovery\/item\/[a-z0-9]+/i.test(location.pathname)) {
    result.openMode = 'route';
  } else if (document.querySelector('.note-detail-mask, .note-container, #noteContainer')) {
    result.openMode = 'popup';
  }

  console.table(result.cardSelectors);
  console.log('[probe-batch-flow] result:', result);
  return result;
})();
