/**
 * 评论图片区探查脚本（在笔记详情页 DevTools Console 运行）
 *
 * 目的：
 * 1. 验证评论区图片选择器
 * 2. 比较原始 src 与去压缩参数后的候选 URL
 * 3. 输出前 20 个样本，供 SELECTORS.md 记录
 */
(function probeCommentMedia() {
  const container = document.querySelector('.comments-container');
  if (!container) {
    console.warn('[probe-comment-media] 未找到评论区容器 .comments-container');
    return null;
  }

  const images = [...container.querySelectorAll('.comment-item img, .comment-item-sub img')];
  const samples = [];
  const seen = new Set();

  function toCandidates(src) {
    if (!src) return [];
    const list = [];
    const add = (value) => {
      if (!value) return;
      const normalized = String(value).trim();
      if (!normalized || list.includes(normalized)) return;
      list.push(normalized);
    };
    const noQuery = src.split('?')[0];
    add(noQuery);
    if (noQuery.includes('!')) add(noQuery.split('!')[0]);
    if (src.includes('x-oss-process=')) {
      add(src.replace(/([?&])x-oss-process=[^&]*/g, '').replace(/[?&]$/, ''));
    }
    if (src.includes('imageView2') || src.includes('imageslim') || src.includes('thumbnail')) {
      add(src.replace(/([?&])(imageView2|imageslim|thumbnail)=[^&]*/g, '').replace(/[?&]$/, ''));
    }
    add(src);
    return list;
  }

  function extractUrlsFromBackground(styleText) {
    if (!styleText || !styleText.includes('url(')) return [];
    const urls = [];
    const re = /url\((['"]?)(.*?)\1\)/g;
    let m;
    while ((m = re.exec(styleText)) !== null) {
      if (m[2]) urls.push(m[2].trim());
    }
    return urls;
  }

  function extractSources(img) {
    const result = [];
    const add = (value) => {
      if (!value) return;
      const normalized = String(value).trim();
      if (!normalized || result.includes(normalized)) return;
      result.push(normalized);
    };
    add(img.currentSrc);
    add(img.src);
    add(img.dataset?.src);
    add(img.dataset?.origin);
    add(img.dataset?.originSrc);
    add(img.getAttribute('data-src'));
    add(img.getAttribute('data-origin'));
    add(img.getAttribute('data-origin-src'));

    const srcset = img.getAttribute('srcset') || '';
    srcset.split(',').forEach((item) => {
      const [url] = item.trim().split(/\s+/);
      add(url);
    });

    let node = img;
    for (let depth = 0; node && depth < 5; depth++) {
      add(node.getAttribute?.('href'));
      const style = node.getAttribute?.('style') || '';
      extractUrlsFromBackground(style).forEach((u) => add(u));
      const dataset = node.dataset || {};
      Object.keys(dataset).forEach((k) => add(dataset[k]));
      node = node.parentElement;
    }
    return result;
  }

  images.forEach((img) => {
    const sources = extractSources(img);
    if (sources.length === 0) return;
    if (img.closest('a.name') || img.closest('.avatar') || img.closest('.author-wrapper')) return;
    const src = sources[0];
    if (seen.has(src)) return;
    seen.add(src);
    const candidates = [];
    sources.forEach((source) => {
      toCandidates(source).forEach((candidate) => {
        if (!candidates.includes(candidate)) candidates.push(candidate);
      });
    });
    samples.push({
      src,
      sources,
      candidates,
      width: img.naturalWidth || img.width || 0,
      height: img.naturalHeight || img.height || 0,
      className: img.className || '',
    });
  });

  const output = {
    url: location.href,
    time: new Date().toISOString(),
    count: samples.length,
    samples: samples.slice(0, 20),
  };

  console.log('[probe-comment-media] result:', output);
  return output;
})();
