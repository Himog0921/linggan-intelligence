// 注入页面读取笔记数据，通过 postMessage 回传给 content script
// IIFE 避免变量名与页面全局作用域冲突
(function() {
  var requestId = new URL(document.currentScript && document.currentScript.src || location.href).searchParams.get('requestId') || '';
  try {
    var raw = window.__INITIAL_STATE__?.note?.noteDetailMap || {};
    var result = {};
    for (var key in raw) {
      if (!raw.hasOwnProperty(key)) continue;
      if (!key || key === 'undefined' || !/^[a-f0-9]{24}$/i.test(key)) continue;
      var value = raw[key];
      result[key] = value?.note || value;
    }
    window.postMessage({ type: 'noteMap', requestId: requestId, data: result }, '*');
  } catch (e) {
    window.postMessage({ type: 'noteMap', requestId: requestId, data: {}, error: e.message }, '*');
  }
})();
