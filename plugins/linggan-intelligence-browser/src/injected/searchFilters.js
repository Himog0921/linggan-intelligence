// 注入页面读取当前生效的搜索筛选，通过 postMessage 回传给 content script。
//
// 为什么必须注入：`__INITIAL_STATE__` 是页面自己的 JS 变量，而 content script 跑在
// Chrome 的隔离世界里，按设计看不见它。此前 `readCurrentXhsSearchFilterSnapshot`
// 直接读 `window.__INITIAL_STATE__`，在真实页面上恒为 undefined——于是每一项都回落成
// 「沿用当前」，把一次真的按最多点赞采回的样本写成了「筛选没生效」。
//
// 这条路与 noteMap.js 是同一套机制，那边已经在真实页面上验证过。
//
// IIFE 避免变量名与页面全局作用域冲突。
(function() {
  var requestId = new URL(document.currentScript && document.currentScript.src || location.href).searchParams.get('requestId') || '';
  // 页面把状态包在 Vue 的 ref 里，取值要穿过 _rawValue / _value。
  function unwrap(value) {
    if (!value || typeof value !== 'object') return value;
    if ('_rawValue' in value) return unwrap(value._rawValue);
    if ('_value' in value) return unwrap(value._value);
    return value;
  }
  try {
    var search = unwrap(unwrap(window.__INITIAL_STATE__ || {}).search) || {};
    var params = unwrap(search.filterParams) || [];
    var raw = {};
    if (Array.isArray(params)) {
      for (var i = 0; i < params.length; i++) {
        var entry = unwrap(params[i]) || {};
        var type = String(entry.type || '').trim();
        if (!type) continue;
        var tags = unwrap(entry.tags) || [];
        // 标签可能是字符串，也可能是带 name/text 的对象——两种都见过。
        raw[type] = (Array.isArray(tags) ? tags : [tags]).map(function(tag) {
          var value = unwrap(tag);
          if (typeof value === 'string') return value;
          return String((value && (value.name || value.text)) || '');
        }).filter(Boolean);
      }
    }
    // `readable` 说明这一次到底读到了没有：空的筛选状态与读不到是两件事，
    // 后者不该被当成「页面上什么都没选」。
    window.postMessage({
      type: 'searchFilters',
      requestId: requestId,
      data: { readable: Boolean(window.__INITIAL_STATE__), raw: raw },
    }, '*');
  } catch (e) {
    window.postMessage({
      type: 'searchFilters',
      requestId: requestId,
      data: { readable: false, raw: {} },
      error: e.message,
    }, '*');
  }
})();
