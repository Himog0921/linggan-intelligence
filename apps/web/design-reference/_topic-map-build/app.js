/* 主题图谱 · 重设计对齐稿 · 演示数据沿用原 demo，全部为示例，不代表真实运行结果。 */
(function () {
  'use strict';

  /* ---------- 图标：ICON-B-001（shared/IntelligenceIcon.tsx 同一几何） ---------- */
  const SHAPES = {
    search: '<g class="feedback-move"><circle cx="10.25" cy="10.25" r="5.75"/><path d="m14.35 14.35 4.9 4.9"/><path class="signal-stroke feedback-trace" pathLength="1" d="M6.5 10.25a3.75 3.75 0 0 1 3.75-3.75"/></g>',
    chevron: '<g class="chapter-chevron"><path d="m6.5 9.5 5.5 5 5.5-5"/></g>',
    external: '<path class="tone-face" d="M5 6h5v13H5z"/><path d="M10.5 5H6a1 1 0 0 0-1 1v12a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-4.5"/><g class="feedback-move"><path d="M11 13l8-8m-5.5 0H19v5.5"/><path class="signal-stroke feedback-trace" pathLength="1" d="M11 13l8-8"/></g>',
    link: '<path class="rear-contour" d="m9.3 13.1 4.2-4.2a3 3 0 0 1 4.25 4.25l-2.5 2.5"/><g class="feedback-move"><path d="m14.7 10.9-4.2 4.2a3 3 0 0 1-4.25-4.25l2.5-2.5"/><path class="signal-stroke" d="m10 14 4-4"/></g>',
    close: '<g class="close-lines"><path d="m7 7 10 10M17 7 7 17"/></g>'
  };
  const icon = name => '<svg class="lgi-icon" data-icon="' + name + '" aria-hidden="true" focusable="false" viewBox="0 0 24 24" width="1em" height="1em">' + (SHAPES[name] || '') + '</svg>';
  function hydrateIcons(root) { (root || document).querySelectorAll('span[data-icon]').forEach(el => { el.outerHTML = icon(el.dataset.icon); }); }
  const $ = id => document.getElementById(id);
  const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
  const fmt = n => n == null ? '—' : Number(n).toLocaleString('en-US');

  /* ---------- 数据（沿用原 demo） ---------- */
  const DOMAIN = { name: 'ADHD', works: 3282, readable: 3267, unassigned: 3242, updated: '11/26 17:44' };
  const TREE = [
    { name: 'ADHD新鲜情境优势与后续维持落差', count: 1, children: [['ADHD对新异与熟悉信息的注意力成本差异', 1], ['ADHD工作安排中启动与运维职责分离', 1]] },
    { name: 'A娃养育如何调整普通鸡娃路径', count: 1, children: [['ADHD孩子先运动再作业如何安排', 1], ['ADHD孩子睡眠优先如何协调作业量', 1], ['A娃如何用单一兴趣带动学科学习', 1], ['A娃家庭如何脱离学校评价标准', 1]] },
    { name: '儿童行为与情绪问题的可落地干预方法', count: 2, children: [['ADHD儿童冲突中用言语回击替代动手', 1]] },
    { name: '儿童行为问题的成因与需求分析', count: 2, children: [['ADHD对立违抗背后的恐惧与情绪成因', 1]] }
  ];
  const SINGLES = [
    ['15岁前大脑可塑性与ADHD干预信心', 1], ['ADHD与读写障碍的“多重天赋后遗症”解释', 1], ['ADHD与销售等即时反馈工作的适配', 1],
    ['ADHD代偿在生育后失效与成人就医标签困境', 1], ['ADHD孩子识字记字困难与家长陪学痛苦', 1], ['孩子上课注意力不集中、发呆的家长求助', 1],
    ['孩子写作业坐不住与家长无奈', 1], ['儿童阅读与书写障碍训练资料的适配选择', 1]
  ];
  const DIST = [['0–20', 1320, false], ['21–99', 801, false], ['100–499', 651, true], ['500–999', 168, true], ['1千–5千', 209, true], ['≥5千', 94, true]];
  const MATERIALS = [
    { title: '时间可视化，看看谁是偷走你手机时间的top1', author: '自律鸭鸭', date: '2026/11/25', type: '待归方向', likes: 2, saves: 0, comments: 0, shares: 0, filter: 'latest', topics: ['__unassigned'],
      excerpt: '每天明明只刷了一会手机，怎么看时间已经过去了三个小时！时间都去哪儿了，看不见也没感觉啊！抓狂，那就来看看鸭鸭帮你记录的手机使用时间吧。',
      quote: '每天明明只刷了一会手机，怎么看时间已经过去了三个小时！时间都去哪儿了，看不见也没感觉啊！', source: '小红书' },
    { title: '出门前总找钥匙？从今晚改', author: '长皮虾', date: '2026/11/25', type: '待归方向', likes: null, saves: null, comments: null, shares: null, filter: 'latest', topics: ['__unassigned'],
      excerpt: '马上就走，有时候只是还没开始找钥匙。包在沙发上，工牌在另一件外套口袋里，伞等到拉开门才想起来。',
      quote: '三个来回之后，电梯已经走了两趟。把指南里的晨间练习压成了今晚就能做的一件事。', source: '小红书' },
    { title: '家长在教导受挫后，如何重新安排日常练习', author: 'ADHD有多难', date: '2026/11/24', type: '家庭训练', likes: 128, saves: 36, comments: 12, shares: 5, filter: 'high', topics: ['儿童行为与情绪问题的可落地干预方法'],
      excerpt: '每天都想把事情做好，但孩子和家长都容易在反复提醒中疲惫。先把练习拆小，再决定什么时候继续。',
      quote: '情绪还是会忍不住暴躁起来。后来我发现，先停下来比继续讲道理更有用。', source: '小红书' },
    { title: '孩子先运动再写作业，家里的一次调整', author: '真正高端的A娃养育', date: '2026/11/23', type: '家庭训练', likes: 56, saves: 18, comments: 7, shares: 2, filter: 'ordinary', topics: ['ADHD孩子先运动再作业如何安排'],
      excerpt: '放学后先运动一会儿，再回来写作业。不是每次都顺利，但家里的冲突少了一些。',
      quote: '我家大盛每天放学以后，先运动30分钟，再回来写作业。', source: '小红书' },
    { title: '三年级孩子在家庭语文学习中的识字困境', author: '读写障碍慢慢变好后', date: '2026/11/22', type: '学习场景', likes: 305, saves: 77, comments: 21, shares: 8, filter: 'high', topics: ['ADHD孩子识字记字困难与家长陪学痛苦'],
      excerpt: '孩子能说出意思，却常常写不出来。每天只安排少量生字，先让他真正理解字形和意思。',
      quote: '每天只吃透3个生字。慢一点，孩子反而愿意继续。', source: '小红书' }
  ];
  const STAGES = [
    { name: '发现与理解', n: 6, pct: '0.2%', q: '发生了什么？怎样理解？' },
    { name: '求助与评估', n: 2, pct: '0.1%', q: '何时求助？如何评估？' },
    { name: '选择支持', n: 2, pct: '0.1%', q: '哪些支持适合当前处境？' },
    { name: '开始实践', n: 5, pct: '0.2%', q: '先试什么？怎样开始？' },
    { name: '长期管理', n: 13, pct: '0.4%', q: '如何调整并持续支持？' }
  ];
  const SCENES = {
    '发现与理解': [
      ['面向英区 ADHD 同伴的线上学习招募', '1 篇独立来源作品 · 1 条定位依据', '寻英区 ADHD 一起线上学习', '一位家长在寻找可以共同学习、相互陪伴的同伴。'],
      ['独自学习难以坚持，朋友只玩乐的日常处境', '1 篇独立来源作品 · 2 条定位依据', '我自己学不下去', '想找一个能一起学习的人，却很难在身边找到。'],
      ['三年级 ADHD 孩子在家庭语文学习中的识字困境', '1 篇独立来源作品 · 2 条定位依据', '注意力缺陷型 ADHD 多动症，三年级了', '家长开始把孩子的学习困难落实到具体的识字场景。'],
      ['七岁小一孩子口头答得出，书写仍做不到', '1 篇独立来源作品 · 2 条定位依据', '已经七岁，上小一的他仍然做不到', '口头表达和书面表达之间出现明显落差。']
    ],
    '求助与评估': [
      ['家长寻找读写困难的专业评估路径', '1 篇独立来源作品 · 2 条定位依据', '到底要不要做一次专业评估？', '家长希望知道下一步应该找谁、看什么。'],
      ['复查改善后仍然不知道如何判断', '1 篇独立来源作品 · 1 条定位依据', '复查结果比之前好，但还是很纠结', '已有检查结果，但缺少理解和比较依据。']
    ],
    '选择支持': [
      ['在家用 Phonics 教三个单词，孩子仍难以独立写出', '1 篇独立来源作品 · 3 条定位依据', '今天用 Phonics 教他拼读，只安排三个单词', '家长在比较不同学习方法是否适合孩子。'],
      ['家长寻找适合孩子的训练资料', '1 篇独立来源作品 · 2 条定位依据', '有没有更适合他的训练材料？', '选择支持方式时，需要考虑孩子真实的学习表现。']
    ],
    '开始实践': [
      ['放学后先运动再写作业', '1 篇独立来源作品 · 1 条定位依据', '我家大盛每天放学以后，先运动30分钟', '家庭开始尝试调整日常任务顺序。'],
      ['每天只安排三个生字的识字练习', '1 篇独立来源作品 · 2 条定位依据', '每天只吃透3个生字', '把练习拆成孩子能够完成的小任务。']
    ],
    '长期管理': [
      ['家长在教导受挫后重新调整目标', '1 篇独立来源作品 · 3 条定位依据', '情绪还是会忍不住暴躁起来', '家庭在反复实践中调整目标和节奏。'],
      ['长期记录孩子的睡眠与学习状态', '1 篇独立来源作品 · 2 条定位依据', '最近开始记录每天的状态', '家长持续观察变化，再调整家庭安排。']
    ]
  };

  /* ---------- 状态 ---------- */
  const SELECTS = {
    platform: { label: '平台', options: ['全部平台', '小红书', '抖音'], value: '全部平台' },
    time: { label: '时间', options: ['全部时间', '近 7 天', '近 30 天', '近 90 天'], value: '全部时间' }
  };
  const state = { topic: '全部方向', tab: 'materials', filter: 'latest', chart: 'bars', view: 'overview', stage: '发现与理解', jmetric: 'main', jdisplay: 'map', collapsed: new Set(), saved: new Set() };
  const UNASSIGNED = '__unassigned';
  const topicCount = () => TREE.reduce((n, g) => n + 1 + g.children.length, 0) + SINGLES.length;
  function findTopic(name) {
    for (const g of TREE) {
      if (g.name === name) return { name, count: g.count, parent: null, children: g.children.length };
      for (const [c, n] of g.children) if (c === name) return { name, count: n, parent: g.name, children: 0 };
    }
    for (const [s, n] of SINGLES) if (s === name) return { name, count: n, parent: null, children: 0 };
    return null;
  }
  function inScope(m) {
    if (state.topic === '全部方向') return true;
    const g = TREE.find(x => x.name === state.topic);
    const names = g ? [g.name, ...g.children.map(c => c[0])] : [state.topic];
    return m.topics.some(t => names.includes(t));
  }
  const scopeTotal = () => state.topic === '全部方向' ? DOMAIN.works : state.topic === UNASSIGNED ? DOMAIN.unassigned : (findTopic(state.topic) || {}).count;
  const pct = (a, b) => { const v = a / b * 100; return v > 0 && v < 0.1 ? '&lt;0.1%' : v.toFixed(1) + '%'; };
  /* 徽章语义：分类＝中性；待归方向＝未知（尚未归类）；高表现＝Signal（与分布图同义）；待核对＝Warning */
  const badge = (text, kind) => '<span class="badge ' + kind + '">' + esc(text) + '</span>';
  const typeBadge = t => t === '待归方向' ? badge('尚未归类', 'unknown') : badge(t, 'category');

  function toast(msg) {
    const t = $('toast'); t.textContent = msg; t.classList.add('show');
    clearTimeout(toast.timer); toast.timer = setTimeout(() => t.classList.remove('show'), 2000);
  }

  /* ---------- C16 FIND 行为：清空 / "/" 快捷键 / 不提交表单 ---------- */
  function wireFind(form, onInput) {
    const input = form.querySelector('input'), kbd = form.querySelector('kbd'), clear = form.querySelector('.clear');
    const sync = () => { const has = !!input.value; kbd.hidden = has; clear.hidden = !has; };
    input.addEventListener('input', () => { sync(); onInput(); });
    clear.addEventListener('click', () => { input.value = ''; sync(); onInput(); input.focus(); });
    form.addEventListener('submit', e => e.preventDefault());
    sync();
  }

  /* ---------- 主题树 ---------- */
  function renderTree() {
    const q = $('treeSearch').value.trim().toLowerCase();
    const hit = s => !q || s.toLowerCase().includes(q);
    const node = (name, count, cls) => '<button class="tm-node' + (cls ? ' ' + cls : '') + (state.topic === name ? ' is-active' : '') + '" type="button" data-topic="' + esc(name) + '"' + (state.topic === name ? ' aria-current="true"' : '') + ' title="' + esc(name) + '"><span class="name">' + esc(name) + '</span><span class="count">' + fmt(count) + '</span></button>';
    let html = '<button class="tm-node tm-plain tm-all' + (state.topic === '全部方向' ? ' is-active' : '') + '" type="button" data-topic="全部方向"><span class="name">全部方向</span><span class="count">' + fmt(DOMAIN.works) + '</span></button>'
      + '<button class="tm-node tm-plain tm-gap' + (state.topic === UNASSIGNED ? ' is-active' : '') + '" type="button" data-topic="' + UNASSIGNED + '" title="全部方向中尚未归入任何主题的材料"><span class="name">其中未归入主题</span><span class="count">' + fmt(DOMAIN.unassigned) + '</span></button>';
    const groups = TREE.filter(g => hit(g.name) || g.children.some(([c]) => hit(c)));
    if (groups.length) {
      html += '<div class="tm-tree-label"><span>主题目录</span><span>作品</span></div><ul>';
      groups.forEach(g => {
        const collapsed = !q && state.collapsed.has(g.name);
        html += '<li class="tm-group"' + (collapsed ? ' data-collapsed' : '') + '><div class="tm-node-row">'
          + '<button class="tm-toggle" type="button" data-toggle="' + esc(g.name) + '" aria-expanded="' + !collapsed + '" aria-label="' + (collapsed ? '展开' : '收起') + esc(g.name) + '">' + icon('chevron') + '</button>'
          + node(g.name, g.count).replace('<span class="name">', '<span></span><span class="name">') + '</div><ul class="tm-children">'
          + g.children.filter(([c]) => !q || hit(c) || hit(g.name)).map(([c, n]) => '<li>' + node(c, n) + '</li>').join('') + '</ul></li>';
      });
      html += '</ul>';
    }
    const singles = SINGLES.filter(([s]) => hit(s));
    if (singles.length) html += '<div class="tm-tree-label"><span>独立主题</span><span></span></div><ul>' + singles.map(([s, n]) => '<li class="tm-leaf">' + node(s, n) + '</li>').join('') + '</ul>';
    if (q && !groups.length && !singles.length) html += '<div class="tm-tree-empty">没有名称包含「' + esc(q) + '」的主题</div>';
    $('treeScroll').innerHTML = html;
    $('treeTotal').textContent = '主题结构 · ' + topicCount() + ' 个';
  }

  /* ---------- 页头 ---------- */
  function renderHead() {
    const t = findTopic(state.topic), path = $('currentPath'), sep = '<span class="sep">/</span>';
    if (state.topic === '全部方向') {
      $('currentTitle').textContent = '全部方向';
      $('currentMeta').innerHTML = DOMAIN.name + sep + '<b>' + fmt(DOMAIN.works) + '</b> 篇观察材料' + sep + '更新于 <b>' + DOMAIN.updated + '</b>';
      path.hidden = true;
    } else if (state.topic === UNASSIGNED) {
      $('currentTitle').textContent = '未归入主题的材料';
      $('currentMeta').innerHTML = DOMAIN.name + sep + '<b>' + fmt(DOMAIN.unassigned) + '</b> 篇，占全部方向 <b>' + pct(DOMAIN.unassigned, DOMAIN.works) + '</b>' + sep + '更新于 <b>' + DOMAIN.updated + '</b>';
      path.hidden = false; path.innerHTML = '<button type="button" data-topic="全部方向">全部方向</button><i>/</i><span>归类覆盖</span>';
    } else if (t) {
      $('currentTitle').textContent = t.name;
      $('currentMeta').innerHTML = DOMAIN.name + sep + '<b>' + fmt(t.count) + '</b> 篇作品' + (t.children ? sep + '<b>' + t.children + '</b> 个子主题' : '') + sep + '更新于 <b>' + DOMAIN.updated + '</b>';
      path.hidden = false;
      path.innerHTML = '<button type="button" data-topic="全部方向">全部方向</button>' + (t.parent ? '<i>/</i><button type="button" data-topic="' + esc(t.parent) + '">' + esc(t.parent) + '</button>' : '') + '<i>/</i><span>' + (t.parent ? '子主题' : '主题') + '</span>';
    }
  }

  /* ---------- 数据模块：4 个读数 + 分布图共用三行网格 ---------- */
  function renderData() {
    const all = state.topic === '全部方向', works = scopeTotal();
    const ro = (k, v, unit, s) => '<div class="tm-readout"><div class="k">' + k + '</div><div class="vw"><div class="v' + (v === '—' ? ' unknown' : '') + '">' + v + (unit && v !== '—' ? '<small>' + unit + '</small>' : '') + '</div><div class="s">' + s + '</div></div></div>';
    const na = '当前范围未提供';
    let html = ro('观察到的内容', fmt(works), '篇', all ? '可读 <b>' + fmt(DOMAIN.readable) + '</b> · 未归类 <b>' + fmt(DOMAIN.unassigned) + '</b><button type="button" data-topic="' + UNASSIGNED + '">查看</button>' : '占全部方向 <b>' + pct(works, DOMAIN.works) + '</b>')
      + ro('已观察创作者', all ? '1,521' : '—', '位', all ? '身份未知 <b>149</b> 篇' : na)
      + ro('样本高表现率', all ? '34.6%' : '—', '', all ? '达标 <b>1,122</b> / 可判定 <b>3,243</b>' : na)
      + ro('点赞中位数', all ? '37' : '—', '', all ? 'P90 <b>921.4</b> · 未知 <b>39</b> 篇' : na);
    const head = '<div class="tm-chart-head"><h2>样本表现<small>小红书 · 点赞分布</small></h2><div class="tm-chart-tools">'
      + (all ? '<div class="tm-legend">' + badge('常规', 'category') + badge('达到已设高表现规则', 'signal') + '</div>' : '')
      + '<div class="vs small" role="group" aria-label="分布呈现方式"><button type="button" data-chart="bars" aria-pressed="' + (state.chart === 'bars') + '">图表</button><button type="button" data-chart="table" aria-pressed="' + (state.chart === 'table') + '">表格</button></div></div></div>';
    let body;
    if (!all) body = '<div class="empty"><strong>当前范围没有可绘制的分布</strong><p>切回「全部方向」查看领域整体的样本表现。</p></div>';
    else if (state.chart === 'table') {
      const total = DIST.reduce((n, d) => n + d[1], 0);
      body = '<table class="tm-dist-table" style="grid-row:span 2"><thead><tr><th>点赞区间</th>' + DIST.map(d => '<th>' + d[0] + '</th>').join('') + '</tr></thead><tbody>'
        + '<tr><td>作品数</td>' + DIST.map(d => '<td class="' + (d[2] ? 'high' : '') + '">' + fmt(d[1]) + '</td>').join('') + '</tr>'
        + '<tr><td>占比</td>' + DIST.map(d => '<td class="' + (d[2] ? 'high' : '') + '">' + (d[1] / total * 100).toFixed(1) + '%</td>').join('') + '</tr></tbody></table>';
    } else {
      const max = Math.max(...DIST.map(d => d[1]));
      body = '<div class="tm-bars">' + DIST.map(d => '<button class="tm-bar' + (d[2] ? ' high' : '') + '" type="button" title="' + d[0] + ' 赞：' + fmt(d[1]) + ' 篇" data-bin="' + d[0] + '"><span class="bv">' + fmt(d[1]) + '</span><span class="bb" style="height:' + Math.max(3, d[1] / max * 78).toFixed(1) + '%"></span></button>').join('') + '</div>'
        + '<div class="tm-bar-labels">' + DIST.map(d => '<span>' + d[0] + '</span>').join('') + '</div>';
    }
    $('dataModule').innerHTML = html + '<div class="tm-chart">' + head + body + '</div>';
  }

  /* ---------- 工作区 ---------- */
  function setWorkHead(title, count, tools) { $('sectionTitle').textContent = title; $('sectionCount').innerHTML = count; $('materialTools').hidden = !tools; }
  function renderMaterials() {
    const q = $('materialSearch').value.trim().toLowerCase();
    const scoped = MATERIALS.map((m, i) => ({ ...m, i })).filter(inScope);
    const rows = scoped.filter(m => (state.filter === 'latest' || m.filter === state.filter) && (!q || (m.title + m.author + m.excerpt).toLowerCase().includes(q)));
    const total = scopeTotal();
    setWorkHead('材料与讨论', '<b>' + fmt(total) + '</b> 篇 · 演示数据含 ' + scoped.length + ' 篇', true);
    const stat = (v, high) => '<span class="tm-stat' + (v == null ? ' unknown' : high ? ' high' : '') + '"' + (v == null ? ' title="未获取到该互动数据"' : '') + '>' + fmt(v) + '</span>';
    if (!rows.length) {
      $('tabContent').innerHTML = scoped.length
        ? '<div class="empty" data-empty-kind="no_match"><strong>没有匹配的材料</strong><p>换个关键词，或切回「最新作品」。</p></div>'
        : '<div class="empty" data-empty-kind="no_material"><strong>这个主题的作品不在演示数据里</strong><p>真实页面会在这里列出该主题下的 ' + fmt(total) + ' 篇作品。</p></div>';
      return;
    }
    $('tabContent').innerHTML = '<div class="tm-list-head" aria-hidden="true"><span></span><span>材料</span><span>赞</span><span>藏</span><span>评</span><span>分享</span><span>操作</span></div>'
      + rows.map(m => {
        const high = m.likes != null && m.likes >= 100, saved = state.saved.has(m.i);
        return '<article class="tm-row">'
          + '<div class="tm-cover" title="封面未载入">封面<br>未载入</div>'
          + '<div class="tm-row-main">'
          + '<h3 class="tm-row-title"><button type="button" data-open="' + m.i + '" title="' + esc(m.title) + '">' + esc(m.title) + '</button></h3>'
          + '<div class="tm-row-meta"><span class="who">' + m.source + ' · ' + esc(m.author) + ' · <span class="num">' + m.date + '</span></span>' + typeBadge(m.type) + (high ? badge('高表现', 'signal') : '') + '</div>'
          + '<p class="tm-row-excerpt">' + esc(m.excerpt) + '</p>'
          + '</div>'
          + stat(m.likes, high) + stat(m.saves) + stat(m.comments) + stat(m.shares)
          + '<div class="tm-row-actions"><button class="text-action" type="button" data-open="' + m.i + '">原文与讨论' + icon('external') + '</button>'
          + '<button class="btn small" type="button" data-save="' + m.i + '" aria-pressed="' + saved + '">' + (saved ? '已保留' : '保留备选') + '</button></div>'
          + '</article>';
      }).join('')
      + '<div class="tm-list-foot"><span>演示数据 · 当前范围 ' + scoped.length + ' 篇</span><span>「—」表示未获取，不等于 0</span></div>';
  }
  function renderStructure() {
    setWorkHead('主题供给分布', '按子主题查看内容供给与表现', false);
    const rows = [['ADHD孩子先运动再作业如何安排', 1, 1], ['ADHD孩子睡眠优先如何协调作业量', 1, 1], ['A娃如何用单一兴趣带动学科学习', 1, 1], ['A娃家庭如何脱离学校评价标准', 1, 1], ['ADHD孩子识字记字困难与家长陪学痛苦', 1, 1]];
    const max = Math.max(...rows.map(r => r[1]));
    $('tabContent').innerHTML = '<table class="tm-table"><thead><tr><th>子主题</th><th>内容供给</th><th class="r">样本高表现率</th><th class="r">创作者</th><th class="r">操作</th></tr></thead><tbody>'
      + rows.map(r => '<tr><td class="strong">' + esc(r[0]) + '</td><td><span class="tm-meter"><i style="--w:' + (r[1] / max * 100) + '%"></i><span class="n">' + r[1] + '</span> 篇</span></td><td class="r muted" title="样本不足或未提供">—</td><td class="r n">' + r[2] + '</td><td class="r"><button class="text-action" type="button" data-topic="' + esc(r[0]) + '">查看材料' + icon('external') + '</button></td></tr>').join('')
      + '</tbody></table><p class="tm-note">「—」表示样本不足或未提供，不等于 0%。</p>';
  }
  function renderPatterns() {
    setWorkHead('内容格局', '内容类型 · 表达方式 · 回答形式', false);
    const groups = [['内容类型', [['方法指南', 32], ['个人故事', 26], ['解释说明', 18], ['清单', 12], ['案例分析', 9]]], ['表达方式', [['亲身经历', 41], ['观点表达', 27], ['经验分享', 19], ['问题提问', 13]]], ['回答形式', [['具体步骤', 35], ['经验建议', 28], ['原因解释', 21], ['互动讨论', 16]]]];
    $('tabContent').innerHTML = '<div class="tm-patterns">' + groups.map(([h, items]) => {
      const max = Math.max(...items.map(v => v[1]));
      return '<section class="tm-pattern"><h3>' + h + '</h3>' + items.map(v => '<div class="tm-pattern-row"><span>' + v[0] + '</span><i style="--w:' + (v[1] / max * 100) + '%"></i><b>' + v[1] + '</b></div>').join('') + '</section>';
    }).join('') + '</div>';
  }
  function renderSources() {
    setWorkHead('来源分布', '已观察创作者 · 按作品数排序', false);
    const rows = [['ADHD有多难', '家庭养育', 28, '3.8%'], ['真正高端的A娃养育', '家庭训练', 22, '3.1%'], ['读写障碍慢慢变好后', '学习场景', 16, '2.4%'], ['自律鸭鸭', '日常管理', 12, '1.8%'], ['长皮虾', '生活经验', 9, '1.2%']];
    const max = rows[0][2];
    $('tabContent').innerHTML = '<table class="tm-table"><thead><tr><th style="width:48px">#</th><th>创作者</th><th>主要方向</th><th>作品数</th><th class="r">占比</th></tr></thead><tbody>'
      + rows.map((r, i) => '<tr><td class="n muted">' + String(i + 1).padStart(2, '0') + '</td><td class="strong">' + esc(r[0]) + '</td><td>' + badge(r[1], 'category') + '</td><td><span class="tm-meter"><i style="--w:' + (r[2] / max * 100) + '%"></i><span class="n">' + r[2] + '</span> 篇</span></td><td class="r n">' + r[3] + '</td></tr>').join('') + '</tbody></table>';
  }
  function renderCandidates() {
    setWorkHead('新方向', '近期出现、尚未并入主题结构的候选方向', false);
    const rows = ['ADHD与读写障碍的“多重天赋后遗症”解释', 'ADHD与销售等即时反馈工作的适配', 'ADHD代偿在生育后失效与成人就医标签困境', 'ADHD儿童单独设校与普通课堂包容之争', 'ADHD儿童发育期看书成瘾与信息索取'];
    $('tabContent').innerHTML = '<div class="tm-candidates">' + rows.map((r, i) => '<div class="tm-candidate"><span class="idx">' + String(i + 1).padStart(2, '0') + '</span><div><strong>' + esc(r) + '</strong><small>1 篇来源作品</small></div>' + badge('待核对', 'warning') + '<button class="btn small" type="button" data-candidate="' + esc(r) + '">查看方向</button></div>').join('') + '</div>';
  }
  function renderOwn() {
    setWorkHead('我方对照与再做', '我方发布历史与外部参考', false);
    $('tabContent').innerHTML = '<div class="empty" data-empty-kind="unavailable_feature"><strong>尚未关联我方账号范围</strong><p>关联已确认的我方账号后，这里对照已发布内容、外部参考与主题覆盖。未关联前不显示任何对照数字。</p><div><button class="btn primary small" type="button" id="ownSetup">设置我方矩阵</button></div></div>';
  }
  const RENDER = { materials: renderMaterials, structure: renderStructure, patterns: renderPatterns, sources: renderSources, candidates: renderCandidates, own: renderOwn };
  function renderTab() { document.querySelectorAll('[data-tab]').forEach(b => b.setAttribute('aria-selected', String(b.dataset.tab === state.tab))); RENDER[state.tab](); }

  /* ---------- 旅程 ---------- */
  function renderJourney() {
    const max = Math.max(...STAGES.map(s => s.n)), involved = state.jmetric === 'involved';
    if (state.jdisplay === 'list') {
      $('stageArea').innerHTML = '<table class="tm-table"><thead><tr><th style="width:56px">阶段</th><th>名称</th><th>关键问题</th><th>' + (involved ? '涉及作品' : '主阶段作品') + '</th><th class="r">占可读作品</th></tr></thead><tbody>'
        + STAGES.map((s, i) => '<tr data-stage="' + s.name + '" class="' + (state.stage === s.name ? 'is-active' : '') + '" style="cursor:pointer"><td class="n">' + String(i + 1).padStart(2, '0') + '</td><td class="strong">' + s.name + '</td><td>' + s.q + '</td><td><span class="tm-meter"><i style="--w:' + (s.n / max * 100) + '%"></i><span class="n">' + s.n + '</span> 篇</span></td><td class="r n">' + s.pct + '</td></tr>').join('') + '</tbody></table>';
    } else {
      $('stageArea').innerHTML = '<div class="tm-stages" role="group" aria-label="旅程阶段">' + STAGES.map((s, i) => '<button class="tm-stage" type="button" data-stage="' + s.name + '" aria-pressed="' + (state.stage === s.name) + '"><span class="idx">' + String(i + 1).padStart(2, '0') + '</span><span class="name">' + s.name + '</span><span class="q">' + s.q + '</span><span class="val">' + s.n + '<small>篇 · ' + s.pct + '</small></span><span class="rel" style="--w:' + (s.n / max * 100) + '%"></span></button>').join('') + '</div>'
        + '<p class="tm-stage-note">' + (involved ? '阶段涉及率：一篇作品可涉及多个阶段' : '主阶段分布：每篇作品只计入一个主阶段') + ' · 细条为阶段间相对比例</p>';
    }
    const rows = SCENES[state.stage] || [];
    $('sceneHeading').innerHTML = esc(state.stage) + '<small>具体场景</small>';
    $('sceneCount').textContent = '本阶段示例场景 ' + rows.length + ' 个';
    $('sceneGrid').innerHTML = rows.map((r, i) => '<article class="tm-scene"><h4>' + esc(r[0]) + '</h4><div class="meta">' + esc(r[1]) + '</div><blockquote>' + esc(r[2]) + '</blockquote><div class="foot"><span>' + esc(r[3]) + '</span><button class="text-action" type="button" data-scene="' + i + '">来源与上下文' + icon('external') + '</button></div></article>').join('');
    document.querySelectorAll('[data-jmetric]').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.jmetric === state.jmetric)));
    document.querySelectorAll('[data-jdisplay]').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.jdisplay === state.jdisplay)));
  }

  /* ---------- C17 选择：弹出层、键盘 ---------- */
  let openSel = null, active = 0;
  function openSelect(btn) {
    const key = btn.dataset.select, cfg = SELECTS[key], menu = $('selectMenu');
    closeSelect(false);
    openSel = btn; active = Math.max(0, cfg.options.indexOf(cfg.value));
    btn.setAttribute('aria-expanded', 'true');
    paintMenu();
    const r = btn.getBoundingClientRect();
    menu.hidden = false;
    menu.style.minWidth = r.width + 'px';
    menu.style.top = (r.bottom + 4) + 'px';
    menu.style.left = Math.min(r.left, window.innerWidth - menu.offsetWidth - 12) + 'px';
  }
  function paintMenu() {
    const cfg = SELECTS[openSel.dataset.select];
    $('selectMenu').setAttribute('aria-label', cfg.label);
    $('selectMenu').innerHTML = cfg.options.map((o, i) => '<div role="option" data-opt="' + i + '" aria-selected="' + (o === cfg.value) + '"' + (i === active ? ' data-active="true"' : '') + '><span>' + o + '</span></div>').join('');
  }
  function closeSelect(refocus) {
    if (!openSel) return;
    openSel.setAttribute('aria-expanded', 'false'); $('selectMenu').hidden = true;
    if (refocus) openSel.focus();
    openSel = null;
  }
  function choose(i) {
    const btn = openSel, cfg = SELECTS[btn.dataset.select];
    cfg.value = cfg.options[i];
    btn.querySelector('.v').textContent = cfg.value;
    closeSelect(true);
    toast(cfg.label + '：' + cfg.value + '（演示）');
  }
  $('selectMenu').addEventListener('click', e => { const o = e.target.closest('[data-opt]'); if (o) choose(Number(o.dataset.opt)); });
  document.addEventListener('keydown', e => {
    if (!openSel) return;
    const n = SELECTS[openSel.dataset.select].options.length;
    if (e.key === 'ArrowDown') { active = (active + 1) % n; paintMenu(); e.preventDefault(); }
    else if (e.key === 'ArrowUp') { active = (active - 1 + n) % n; paintMenu(); e.preventDefault(); }
    else if (e.key === 'Home') { active = 0; paintMenu(); e.preventDefault(); }
    else if (e.key === 'End') { active = n - 1; paintMenu(); e.preventDefault(); }
    else if (e.key === 'Enter' || e.key === ' ') { choose(active); e.preventDefault(); }
    else if (e.key === 'Escape') { closeSelect(true); e.preventDefault(); e.stopPropagation(); }
    else if (e.key === 'Tab') closeSelect(false);
  }, true);
  document.addEventListener('pointerdown', e => { if (openSel && !openSel.contains(e.target) && !$('selectMenu').contains(e.target)) closeSelect(false); }, true);
  $('main').addEventListener('scroll', () => closeSelect(false));

  /* ---------- 检查器 ---------- */
  let lastFocus = null;
  function openInspector(kind, title, quote, summary, meta) {
    lastFocus = document.activeElement;
    $('inspKind').textContent = kind; $('inspTitle').textContent = title;
    const cell = (k, v, cls) => '<div><span>' + k + '</span><strong class="' + (cls || '') + (v == null ? ' unknown' : '') + '">' + (v == null ? '未获取' : esc(v)) + '</strong></div>';
    $('inspBody').innerHTML = '<section class="tm-insp-sec"><h3>原文片段</h3><blockquote class="tm-insp-quote">' + esc(quote) + '</blockquote></section>'
      + '<section class="tm-insp-sec"><h3>内容摘要</h3><p>' + esc(summary) + '</p></section>'
      + (meta.stats ? '<section class="tm-insp-sec"><h3>互动数据</h3><div class="tm-insp-grid">' + cell('点赞', meta.likes == null ? null : fmt(meta.likes), 'n') + cell('收藏', meta.saves == null ? null : fmt(meta.saves), 'n') + cell('评论', meta.comments == null ? null : fmt(meta.comments), 'n') + cell('分享', meta.shares == null ? null : fmt(meta.shares), 'n') + '</div></section>' : '')
      + '<section class="tm-insp-sec"><h3>材料信息</h3><div class="tm-insp-grid two">' + cell('来源平台', meta.source) + cell('发布时间', meta.date) + cell('作者', meta.author) + cell('材料状态', '可阅读') + '</div></section>';
    $('inspector').classList.add('open'); $('inspector').setAttribute('aria-hidden', 'false'); $('scrim').classList.add('open');
    $('inspClose').focus();
  }
  function closeInspector() {
    $('inspector').classList.remove('open'); $('inspector').setAttribute('aria-hidden', 'true'); $('scrim').classList.remove('open');
    if (lastFocus && lastFocus.isConnected) lastFocus.focus();
  }

  function selectTopic(name) { state.topic = name; renderTree(); renderHead(); renderData(); renderTab(); $('main').scrollTo({ top: 0 }); }
  function setView(v) {
    state.view = v;
    document.querySelectorAll('button[data-view]').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.view === v)));
    $('overviewPanel').hidden = v !== 'overview'; $('journeyPanel').hidden = v !== 'journey';
    $('topic-map-root').dataset.view = v;
    if (v === 'journey') renderJourney();
  }

  /* ---------- 事件 ---------- */
  document.addEventListener('click', e => {
    const el = e.target.closest('button,[data-stage],a');
    if (!el) return;
    if (el.dataset.select) { openSel === el ? closeSelect(true) : openSelect(el); return; }
    if (el.dataset.toggle) { const g = el.dataset.toggle; state.collapsed.has(g) ? state.collapsed.delete(g) : state.collapsed.add(g); renderTree(); return; }
    if (el.dataset.topic) { selectTopic(el.dataset.topic); return; }
    if (el.dataset.tab) { state.tab = el.dataset.tab; renderTab(); return; }
    if (el.dataset.filter) { state.filter = el.dataset.filter; document.querySelectorAll('[data-filter]').forEach(b => b.setAttribute('aria-pressed', String(b === el))); renderMaterials(); return; }
    if (el.dataset.chart) { state.chart = el.dataset.chart; renderData(); return; }
    if (el.dataset.view) { setView(el.dataset.view); return; }
    if (el.dataset.jmetric) { state.jmetric = el.dataset.jmetric; renderJourney(); return; }
    if (el.dataset.jdisplay) { state.jdisplay = el.dataset.jdisplay; renderJourney(); return; }
    if (el.dataset.stage) { state.stage = el.dataset.stage; renderJourney(); return; }
    if (el.dataset.open) { const m = MATERIALS[Number(el.dataset.open)]; openInspector('原文与讨论', m.title, m.quote, m.excerpt, { ...m, stats: true }); return; }
    if (el.dataset.scene) { const r = SCENES[state.stage][Number(el.dataset.scene)]; openInspector(state.stage + ' · 场景', r[0], r[2], r[3], { source: '小红书', author: null, date: null }); return; }
    if (el.dataset.save) { const i = Number(el.dataset.save); state.saved.has(i) ? state.saved.delete(i) : state.saved.add(i); renderMaterials(); toast(state.saved.has(i) ? '已保留为备选（演示，未写入）' : '已取消保留'); return; }
    if (el.dataset.bin) { toast('将按 ' + el.dataset.bin + ' 赞筛选材料（演示）'); return; }
    if (el.dataset.candidate) { toast('打开方向详情：' + el.dataset.candidate); return; }
    switch (el.id) {
      case 'inspClose': case 'inspCancel': closeInspector(); break;
      case 'inspLocate': toast('将定位到原始材料（演示）'); break;
      case 'matrixButton': toast('打开主题 × 旅程矩阵（演示）'); break;
      case 'ownSetup': toast('前往我方账号设置（演示）'); break;
    }
  });
  $('scrim').addEventListener('click', closeInspector);
  document.addEventListener('keydown', e => {
    if (e.key === 'Escape' && $('inspector').classList.contains('open')) closeInspector();
    if (e.key === '/' && !e.ctrlKey && !e.metaKey && !e.altKey && !e.isComposing) {
      const t = e.target;
      if (t instanceof HTMLElement && (t.closest('input,textarea,select,[role=listbox]') || t.isContentEditable)) return;
      e.preventDefault(); (state.view === 'overview' && state.tab === 'materials' ? $('materialSearch') : $('treeSearch')).focus();
    }
  });
  wireFind($('treeFind'), renderTree);
  wireFind($('materialFind'), renderMaterials);

  hydrateIcons();
  selectTopic('全部方向');
  window.__tm = { selectTopic, setView, state, openInspector: () => document.querySelector('[data-open="2"]').click(), openSelect: () => openSelect(document.querySelector('[data-select=platform]')) };
})();
