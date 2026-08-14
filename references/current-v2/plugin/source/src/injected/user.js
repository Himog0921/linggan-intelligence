// 注入页面读取博主主页数据，通过 postMessage 回传给 content script
// userPageData 包含当前访问的博主的 interactions、tags、basicInfo
(function() {
  try {
    var userState = window.__INITIAL_STATE__?.user || {};
    window.postMessage({
      type: 'user',
      data: {
        userPageData: userState.userPageData?._rawValue || userState.userPageData || {},
        userInfo: userState.userInfo?._rawValue || userState.userInfo || {},
      }
    }, '*');
  } catch (e) {
    window.postMessage({
      type: 'user',
      data: {
        userPageData: {},
        userInfo: {},
      },
      error: e.message,
    }, '*');
  }
})();
