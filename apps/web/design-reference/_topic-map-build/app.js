/* 主题图谱 · 重设计对齐稿 · 演示数据沿用原 demo，全部为示例，不代表真实运行结果。 */
(function () {
  'use strict';

  /* ---------- 图标（Lucide 线框，与 B-04 稿同源） ---------- */
  const ICONS = {
    Search: [['path', { d: 'm21 21-4.34-4.34' }], ['circle', { cx: '11', cy: '11', r: '8' }]],
    X: [['path', { d: 'M18 6 6 18' }], ['path', { d: 'm6 6 12 12' }]],
    ArrowUpRight: [['path', { d: 'M7 7h10v10' }], ['path', { d: 'M7 17 17 7' }]],
    ChevronDown: [['path', { d: 'm6 9 6 6 6-6' }]],
    Image: [['rect', { width: '18', height: '18', x: '3', y: '3', rx: '1' }], ['circle', { cx: '9', cy: '9', r: '2' }], ['path', { d: 'm21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21' }]],
    Link: [['path', { d: 'M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71' }], ['path', { d: 'M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71' }]]
  };
  function icon(name, size) {
    const s = size || 16;
    const parts = (ICONS[name] || []).map(([tag, attrs]) => '<' + tag + ' ' + Object.entries(attrs).map(([k, v]) => k + '="' + v + '"').join(' ') + '/>').join('');
    return '<svg width="' + s + '" height="' + s + '" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">' + parts + '</svg>';
  }
  function hydrateIcons(root) {
    (root || document).querySelectorAll('span[data-icon]').forEach(el => {
      el.outerHTML = icon(el.dataset.icon, Number(el.dataset.size) || 16);
    });
  }
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
  const state = { topic: '全部方向', parent: null, tab: 'materials', filter: 'latest', chart: 'bars', view: 'overview', stage: '发现与理解', jmetric: 'main', jdisplay: 'map', collapsed: new Set(), saved: new Set() };
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
  const pct = (a, b) => { const v = a / b * 100; return v > 0 && v < 0.1 ? '&lt;0.1%' : v.toFixed(1) + '%'; };

  /* ---------- 反馈 ---------- */
  function toast(msg) {
    const t = $('toast'); t.textContent = msg; t.classList.add('show');
    clearTimeout(toast.timer); toast.timer = setTimeout(() => t.classList.remove('show'), 2000);
  }

  /* ---------- 主题树 ---------- */
  function renderTree() {
    const q = $('treeSearch').value.trim().toLowerCase();
    const hit = s => !q || s.toLowerCase().includes(q);
    const node = (name, count, extra) => '<button class="tm-node' + (state.topic === name ? ' is-active' : '') + '" type="button" data-topic="' + esc(name) + '"' + (state.topic === name ? ' aria-current="true"' : '') + ' title="' + esc(name) + '">' + (extra || '') + '<span class="name">' + esc(name) + '</span><span class="count">' + fmt(count) + '</span></button>';
    let html = '<button class="tm-node tm-all' + (state.topic === '全部方向' ? ' is-active' : '') + '" type="button" data-topic="全部方向"><span class="name">全部方向</span><span class="count">' + fmt(DOMAIN.works) + '</span></button>';
    const groups = TREE.filter(g => hit(g.name) || g.children.some(([c]) => hit(c)));
    if (groups.length) {
      html += '<div class="tm-tree-label"><span>主题目录</span><span>作品</span></div><ul>';
      groups.forEach(g => {
        const collapsed = !q && state.collapsed.has(g.name);
        html += '<li class="tm-group"' + (collapsed ? ' data-collapsed' : '') + '><div class="tm-node-row">'
          + '<button class="tm-toggle" type="button" data-toggle="' + esc(g.name) + '" aria-expanded="' + !collapsed + '" aria-label="' + (collapsed ? '展开' : '收起') + esc(g.name) + '">' + icon('ChevronDown', 12) + '</button>'
          + node(g.name, g.count, '<span></span>') + '</div><ul class="tm-children">'
          + g.children.filter(([c]) => !q || hit(c) || hit(g.name)).map(([c, n]) => '<li>' + node(c, n) + '</li>').join('') + '</ul></li>';
      });
      html += '</ul>';
    }
    const singles = SINGLES.filter(([s]) => hit(s));
    if (singles.length) {
      html += '<div class="tm-tree-label"><span>独立主题</span><span></span></div><ul>' + singles.map(([s, n]) => '<li class="tm-leaf">' + node(s, n) + '</li>').join('') + '</ul>';
    }
    if (q && !groups.length && !singles.length) html += '<div class="tm-tree-empty">没有名称包含「' + esc(q) + '」的主题</div>';
    $('treeScroll').innerHTML = html;
    $('treeTotal').textContent = '主题结构 · ' + topicCount() + ' 个';
    $('treeClear').hidden = !q;
    $('unassigned').classList.toggle('is-active', state.topic === '__unassigned');
  }

  /* ---------- 页头 ---------- */
  function renderHead() {
    const t = findTopic(state.topic);
    const path = $('currentPath');
    if (state.topic === '全部方向') {
      $('currentTitle').textContent = '全部方向';
      $('currentMeta').innerHTML = DOMAIN.name + '<span class="sep">/</span><b>' + fmt(DOMAIN.works) + '</b> 篇观察材料<span class="sep">/</span>更新于 <b>' + DOMAIN.updated + '</b>';
      path.hidden = true;
    } else if (state.topic === '__unassigned') {
      $('currentTitle').textContent = '尚未归类材料';
      $('currentMeta').innerHTML = DOMAIN.name + '<span class="sep">/</span><b>' + fmt(DOMAIN.unassigned) + '</b> 篇未归入任何主题';
      path.hidden = false; path.innerHTML = '<button type="button" data-topic="全部方向">全部方向</button><i>/</i><span>归类覆盖</span>';
    } else if (t) {
      $('currentTitle').textContent = t.name;
      $('currentMeta').innerHTML = DOMAIN.name + '<span class="sep">/</span><b>' + fmt(t.count) + '</b> 篇作品' + (t.children ? '<span class="sep">/</span><b>' + t.children + '</b> 个子主题' : '') + '<span class="sep">/</span>更新于 <b>' + DOMAIN.updated + '</b>';
      path.hidden = false;
      path.innerHTML = '<button type="button" data-topic="全部方向">全部方向</button>' + (t.parent ? '<i>/</i><button type="button" data-topic="' + esc(t.parent) + '">' + esc(t.parent) + '</button>' : '') + '<i>/</i><span>' + (t.parent ? '子主题' : '主题') + '</span>';
    }
  }

  /* ---------- 概览读数 ---------- */
  function renderMetrics() {
    const all = state.topic === '全部方向';
    const t = findTopic(state.topic);
    const works = all ? DOMAIN.works : state.topic === '__unassigned' ? DOMAIN.unassigned : t ? t.count : null;
    const covered = (DOMAIN.works - DOMAIN.unassigned) / DOMAIN.works * 100;
    const unknownSub = '<div class="tm-metric-sub">当前范围未提供</div>';
    const cells = [
      { title: '观察到的内容', value: fmt(works), unit: '篇',
        sub: all ? '<div class="tm-metric-sub"><span>可读 <b>' + fmt(DOMAIN.readable) + '</b></span></div>'
          + '<div class="tm-rail" title="已归入主题的材料占比"><i style="width:' + covered.toFixed(2) + '%"></i></div>'
          + '<div class="tm-rail-note"><span>尚未归类 <b class="num">' + fmt(DOMAIN.unassigned) + '</b> 篇</span><button type="button" data-topic="__unassigned">查看</button></div>'
          : '<div class="tm-metric-sub"><span>占全部方向 <b>' + pct(works, DOMAIN.works) + '</b></span></div>' },
      { title: '已观察创作者', value: all ? '1,521' : '—', unit: all ? '位' : '',
        sub: all ? '<div class="tm-metric-sub"><span>身份未知 <b>149</b> 篇</span></div>' : unknownSub },
      { title: '样本高表现率', value: all ? '34.6%' : '—', unit: '',
        sub: all ? '<div class="tm-metric-sub"><span>达标 <b>1,122</b></span><span>可判定 <b>3,243</b></span></div><div class="tm-rail"><i class="signal" style="width:34.6%"></i></div>' : unknownSub },
      { title: '点赞中位数', value: all ? '37' : '—', unit: '',
        sub: all ? '<div class="tm-metric-sub"><span>P90 <b>921.4</b></span><span>未知 <b>39</b> 篇</span></div>' : unknownSub }
    ];
    $('metrics').innerHTML = cells.map(c => '<div class="tm-metric"><div class="tm-metric-title">' + c.title + '</div><div class="tm-metric-value' + (c.value === '—' ? ' is-unknown' : '') + '">' + c.value + (c.unit ? '<small>' + c.unit + '</small>' : '') + '</div>' + c.sub + '</div>').join('');
  }

  /* ---------- 点赞分布 ---------- */
  function renderDist() {
    const box = $('distBody');
    if (state.topic !== '全部方向') {
      box.innerHTML = '<div class="tm-empty"><strong>当前范围没有可绘制的分布</strong>切回「全部方向」查看领域整体的样本表现。</div>';
      return;
    }
    if (state.chart === 'table') {
      const total = DIST.reduce((n, d) => n + d[1], 0);
      box.innerHTML = '<table class="tm-dist-table"><thead><tr><th>点赞区间</th><th class="r">作品数</th><th class="r">占比</th><th>高表现规则</th></tr></thead><tbody>'
        + DIST.map(d => '<tr><td>' + d[0] + '</td><td class="r">' + fmt(d[1]) + '</td><td class="r">' + (d[1] / total * 100).toFixed(1) + '%</td><td class="' + (d[2] ? 'high' : '') + '">' + (d[2] ? '达到' : '未达到') + '</td></tr>').join('') + '</tbody></table>';
      return;
    }
    const max = Math.max(...DIST.map(d => d[1]));
    box.innerHTML = '<div class="tm-bars" role="list">' + DIST.map(d => '<button class="tm-bar' + (d[2] ? ' high' : '') + '" type="button" role="listitem" title="' + d[0] + ' 赞：' + fmt(d[1]) + ' 篇" data-bin="' + d[0] + '"><span class="v">' + fmt(d[1]) + '</span><span class="b" style="height:' + Math.max(1.5, d[1] / max * 86).toFixed(1) + '%"></span></button>').join('') + '</div>'
      + '<div class="tm-bar-labels">' + DIST.map(d => '<span>' + d[0] + '</span>').join('') + '</div>';
  }

  /* ---------- 工作区 ---------- */
  function setWorkHead(title, count, tools) {
    $('sectionTitle').textContent = title; $('sectionCount').innerHTML = count; $('materialTools').hidden = !tools;
  }
  function renderMaterials() {
    const q = $('materialSearch').value.trim().toLowerCase();
    const scoped = MATERIALS.map((m, i) => ({ ...m, i })).filter(inScope);
    const rows = scoped.filter(m => (state.filter === 'latest' || m.filter === state.filter) && (!q || (m.title + m.author + m.excerpt).toLowerCase().includes(q)));
    const total = state.topic === '全部方向' ? DOMAIN.works : state.topic === '__unassigned' ? DOMAIN.unassigned : (findTopic(state.topic) || {}).count;
    const scope = '<b>' + fmt(total) + '</b> 篇 · 演示数据含 ' + scoped.length + ' 篇';
    setWorkHead('材料与讨论', scope, true);
    const stat = (label, v, high) => '<span>' + label + ' <b class="' + (v == null ? 'unknown' : high ? 'high' : '') + '"' + (v == null ? ' title="未获取到该互动数据"' : '') + '>' + fmt(v) + '</b></span>';
    $('tabContent').innerHTML = rows.length ? '<div class="tm-list">' + rows.map(m => {
      const high = m.likes != null && m.likes >= 100;
      const saved = state.saved.has(m.i);
      return '<article class="tm-row">'
        + '<div class="tm-cover" aria-label="封面未载入">' + icon('Image', 20) + '<span>封面<br>未载入</span></div>'
        + '<div class="tm-row-main">'
        + '<h3 class="tm-row-title"><button type="button" data-open="' + m.i + '">' + esc(m.title) + '</button></h3>'
        + '<div class="tm-row-meta"><span>' + m.source + '</span><i class="dot"></i><span>' + esc(m.author) + '</span><i class="dot"></i><span class="num">' + m.date + '</span>'
        + '<span class="tag' + (m.type === '待归方向' ? ' pending' : '') + '">' + m.type + '</span>' + (high ? '<span class="tag high">高表现</span>' : '') + '</div>'
        + '<p class="tm-row-excerpt">' + esc(m.excerpt) + '</p>'
        + '<div class="tm-row-stats">' + stat('赞', m.likes, high) + stat('藏', m.saves) + stat('评', m.comments) + stat('分享', m.shares) + '</div>'
        + '</div>'
        + '<div class="tm-row-actions"><button class="text-action" type="button" data-open="' + m.i + '">阅读原文与讨论' + icon('ArrowUpRight', 12) + '</button>'
        + '<button class="btn small" type="button" data-save="' + m.i + '" aria-pressed="' + saved + '">' + (saved ? '已保留为备选' : '人工保留备选') + '</button></div>'
        + '</article>';
    }).join('') + '</div><div class="tm-list-foot"><span>演示数据 · 当前范围 ' + scoped.length + ' 篇</span><span>互动数据「—」表示未获取，不等于 0</span></div>'
      : scoped.length ? '<div class="tm-empty"><strong>没有匹配的材料</strong>换个关键词，或切回「最新作品」。</div>'
      : '<div class="tm-empty"><strong>这个主题的作品不在演示数据里</strong>真实页面会在这里列出该主题下的 ' + fmt(total) + ' 篇作品。</div>';
  }
  function renderStructure() {
    setWorkHead('主题供给分布', '按子主题查看内容供给与表现', false);
    const rows = [['ADHD孩子先运动再作业如何安排', 1, 1], ['ADHD孩子睡眠优先如何协调作业量', 1, 1], ['A娃如何用单一兴趣带动学科学习', 1, 1], ['A娃家庭如何脱离学校评价标准', 1, 1], ['ADHD孩子识字记字困难与家长陪学痛苦', 1, 1]];
    const max = Math.max(...rows.map(r => r[1]));
    $('tabContent').innerHTML = '<table class="tm-table"><thead><tr><th>子主题</th><th>内容供给</th><th class="r">样本高表现率</th><th class="r">创作者</th><th class="r">操作</th></tr></thead><tbody>'
      + rows.map(r => '<tr><td class="strong">' + esc(r[0]) + '</td><td><span class="tm-meter"><i style="--w:' + (r[1] / max * 100) + '%"></i><span class="n">' + r[1] + '</span> 篇</span></td><td class="r muted" title="样本不足以计算">—</td><td class="r n">' + r[2] + '</td><td class="r"><button class="text-action light" type="button" data-topic="' + esc(r[0]) + '">查看材料' + icon('ArrowUpRight', 12) + '</button></td></tr>').join('')
      + '</tbody></table><p class="tm-table-note">「—」表示样本不足或未提供，不等于 0%。</p>';
  }
  function renderPatterns() {
    setWorkHead('内容格局', '内容类型 · 表达方式 · 回答形式', false);
    const groups = [['内容类型', [['方法指南', 32], ['个人故事', 26], ['解释说明', 18], ['清单', 12], ['案例分析', 9]]], ['表达方式', [['亲身经历', 41], ['观点表达', 27], ['经验分享', 19], ['问题提问', 13]]], ['回答形式', [['具体步骤', 35], ['经验建议', 28], ['原因解释', 21], ['互动讨论', 16]]]];
    $('tabContent').innerHTML = '<div class="tm-patterns">' + groups.map(([h, items]) => {
      const max = Math.max(...items.map(v => v[1]));
      return '<section class="tm-pattern"><h3>' + h + '</h3>' + items.map(v => '<div class="tm-pattern-row"><span>' + v[0] + '</span><b>' + v[1] + '</b><i style="--w:' + (v[1] / max * 100) + '%"></i></div>').join('') + '</section>';
    }).join('') + '</div>';
  }
  function renderSources() {
    setWorkHead('来源分布', '已观察创作者 · 按作品数排序', false);
    const rows = [['ADHD有多难', '家庭养育', 28, '3.8%'], ['真正高端的A娃养育', '家庭训练', 22, '3.1%'], ['读写障碍慢慢变好后', '学习场景', 16, '2.4%'], ['自律鸭鸭', '日常管理', 12, '1.8%'], ['长皮虾', '生活经验', 9, '1.2%']];
    const max = rows[0][2];
    $('tabContent').innerHTML = '<table class="tm-table"><thead><tr><th style="width:40px">#</th><th>创作者</th><th>主要方向</th><th>作品数</th><th class="r">占比</th></tr></thead><tbody>'
      + rows.map((r, i) => '<tr><td class="n muted">' + String(i + 1).padStart(2, '0') + '</td><td class="strong">' + esc(r[0]) + '</td><td>' + r[1] + '</td><td><span class="tm-meter"><i style="--w:' + (r[2] / max * 100) + '%"></i><span class="n">' + r[2] + '</span> 篇</span></td><td class="r n">' + r[3] + '</td></tr>').join('') + '</tbody></table>';
  }
  function renderCandidates() {
    setWorkHead('新方向', '近期出现、尚未并入主题结构的候选方向', false);
    const rows = ['ADHD与读写障碍的“多重天赋后遗症”解释', 'ADHD与销售等即时反馈工作的适配', 'ADHD代偿在生育后失效与成人就医标签困境', 'ADHD儿童单独设校与普通课堂包容之争', 'ADHD儿童发育期看书成瘾与信息索取'];
    $('tabContent').innerHTML = '<div class="tm-candidates">' + rows.map((r, i) => '<div class="tm-candidate"><span class="idx">' + String(i + 1).padStart(2, '0') + '</span><div><strong>' + esc(r) + '</strong><small>1 篇来源作品</small></div><span class="tag warn">待核对</span><button class="btn small" type="button" data-candidate="' + esc(r) + '">查看方向</button></div>').join('') + '</div>';
  }
  function renderOwn() {
    setWorkHead('我方对照与再做', '我方发布历史与外部参考', false);
    $('tabContent').innerHTML = '<div class="tm-blank"><div class="mark">' + icon('Link', 18) + '</div><div><h3>尚未关联我方账号范围</h3><p>关联已确认的我方账号后，这里对照已发布内容、外部参考与主题覆盖。未关联前不显示任何对照数字。</p><button class="btn primary small" type="button" id="ownSetup">设置我方矩阵</button></div></div>';
  }
  const RENDER = { materials: renderMaterials, structure: renderStructure, patterns: renderPatterns, sources: renderSources, candidates: renderCandidates, own: renderOwn };
  function renderTab() {
    document.querySelectorAll('[data-tab]').forEach(b => b.setAttribute('aria-selected', String(b.dataset.tab === state.tab)));
    RENDER[state.tab]();
  }

  /* ---------- 旅程 ---------- */
  function renderJourney() {
    const max = Math.max(...STAGES.map(s => s.n));
    const involved = state.jmetric === 'involved';
    if (state.jdisplay === 'list') {
      $('stageArea').innerHTML = '<table class="tm-table"><thead><tr><th style="width:56px">阶段</th><th>名称</th><th>关键问题</th><th>' + (involved ? '涉及作品' : '主阶段作品') + '</th><th class="r">占可读作品</th></tr></thead><tbody>'
        + STAGES.map((s, i) => '<tr data-stage="' + s.name + '" style="cursor:pointer' + (state.stage === s.name ? ';background:var(--lgi-canvas-low)' : '') + '"><td class="n">' + String(i + 1).padStart(2, '0') + '</td><td class="strong">' + s.name + '</td><td>' + s.q + '</td><td><span class="tm-meter"><i style="--w:' + (s.n / max * 100) + '%"></i><span class="n">' + s.n + '</span> 篇</span></td><td class="r n">' + s.pct + '</td></tr>').join('') + '</tbody></table>';
    } else {
      $('stageArea').innerHTML = '<div class="tm-stages" role="group" aria-label="旅程阶段">' + STAGES.map((s, i) => '<button class="tm-stage" type="button" data-stage="' + s.name + '" aria-pressed="' + (state.stage === s.name) + '"><span class="idx">' + String(i + 1).padStart(2, '0') + '</span><span class="name">' + s.name + '</span><span class="q">' + s.q + '</span><span class="val">' + s.n + '<small>篇 · ' + s.pct + '</small></span><span class="rel" style="--w:' + (s.n / max * 100) + '%"></span></button>').join('') + '</div>'
        + '<p class="tm-stage-note">' + (involved ? '阶段涉及率：一篇作品可涉及多个阶段' : '主阶段分布：每篇作品只计入一个主阶段') + ' · 细条为阶段间相对比例</p>';
    }
    const rows = SCENES[state.stage] || [];
    $('sceneHeading').innerHTML = esc(state.stage) + '<small>具体场景</small>';
    $('sceneCount').textContent = '本阶段示例场景 ' + rows.length + ' 个';
    $('sceneGrid').innerHTML = rows.map((r, i) => '<article class="tm-scene"><h4>' + esc(r[0]) + '</h4><div class="meta">' + esc(r[1]) + '</div><blockquote>' + esc(r[2]) + '</blockquote><div class="foot"><span>' + esc(r[3]) + '</span><button class="text-action light" type="button" data-scene="' + i + '">阅读来源与上下文' + icon('ArrowUpRight', 12) + '</button></div></article>').join('');
    document.querySelectorAll('[data-jmetric]').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.jmetric === state.jmetric)));
    document.querySelectorAll('[data-jdisplay]').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.jdisplay === state.jdisplay)));
  }

  /* ---------- 检查器 ---------- */
  let lastFocus = null;
  function openInspector(kind, title, quote, summary, meta) {
    lastFocus = document.activeElement;
    $('inspKind').textContent = kind; $('inspTitle').textContent = title;
    const cell = (k, v, cls) => '<div><span>' + k + '</span><strong class="' + (cls || '') + (v == null ? ' unknown' : '') + '">' + (v == null ? '未获取' : esc(v)) + '</strong></div>';
    $('inspBody').innerHTML = '<section class="tm-insp-sec"><h3>原文片段</h3><blockquote class="tm-insp-quote">' + esc(quote) + '</blockquote></section>'
      + '<section class="tm-insp-sec"><h3>内容摘要</h3><p>' + esc(summary) + '</p></section>'
      + '<section class="tm-insp-sec"><h3>材料信息</h3><div class="tm-insp-grid">' + cell('来源平台', meta.source) + cell('发布时间', meta.date, 'n') + cell('作者', meta.author) + cell('材料状态', '可阅读') + '</div></section>'
      + (meta.stats ? '<section class="tm-insp-sec"><h3>互动数据</h3><div class="tm-insp-grid">' + cell('点赞', meta.likes == null ? null : fmt(meta.likes), 'n') + cell('收藏', meta.saves == null ? null : fmt(meta.saves), 'n') + cell('评论', meta.comments == null ? null : fmt(meta.comments), 'n') + cell('分享', meta.shares == null ? null : fmt(meta.shares), 'n') + '</div></section>' : '');
    $('inspector').classList.add('open'); $('inspector').setAttribute('aria-hidden', 'false'); $('scrim').classList.add('open');
    $('inspClose').focus();
  }
  function closeInspector() {
    $('inspector').classList.remove('open'); $('inspector').setAttribute('aria-hidden', 'true'); $('scrim').classList.remove('open');
    if (lastFocus && lastFocus.isConnected) lastFocus.focus();
  }

  /* ---------- 选择主题 ---------- */
  function selectTopic(name) {
    state.topic = name;
    renderTree(); renderHead(); renderMetrics(); renderDist(); renderTab();
    $('main').scrollTo({ top: 0 });
  }
  function setView(v) {
    state.view = v;
    document.querySelectorAll('[data-view]').forEach(b => { if (b.tagName === 'BUTTON') b.setAttribute('aria-pressed', String(b.dataset.view === v)); });
    $('overviewPanel').hidden = v !== 'overview'; $('journeyPanel').hidden = v !== 'journey';
    $('topic-map-root').dataset.view = v;
    if (v === 'journey') renderJourney();
  }

  /* ---------- 事件 ---------- */
  document.addEventListener('click', e => {
    const el = e.target.closest('button,[data-stage],a');
    if (!el) return;
    if (el.dataset.toggle) {
      const g = el.dataset.toggle;
      state.collapsed.has(g) ? state.collapsed.delete(g) : state.collapsed.add(g);
      renderTree(); return;
    }
    if (el.dataset.topic) { selectTopic(el.dataset.topic); return; }
    if (el.dataset.tab) { state.tab = el.dataset.tab; renderTab(); return; }
    if (el.dataset.filter) { state.filter = el.dataset.filter; document.querySelectorAll('[data-filter]').forEach(b => b.setAttribute('aria-pressed', String(b === el))); renderMaterials(); return; }
    if (el.dataset.chart) { state.chart = el.dataset.chart; document.querySelectorAll('[data-chart]').forEach(b => b.setAttribute('aria-pressed', String(b === el))); renderDist(); return; }
    if (el.dataset.view && el.tagName === 'BUTTON') { setView(el.dataset.view); return; }
    if (el.dataset.jmetric) { state.jmetric = el.dataset.jmetric; renderJourney(); return; }
    if (el.dataset.jdisplay) { state.jdisplay = el.dataset.jdisplay; renderJourney(); return; }
    if (el.dataset.stage) { state.stage = el.dataset.stage; renderJourney(); return; }
    if (el.dataset.open) { const m = MATERIALS[Number(el.dataset.open)]; openInspector('原文与讨论', m.title, m.quote, m.excerpt, { ...m, stats: true }); return; }
    if (el.dataset.scene) { const r = SCENES[state.stage][Number(el.dataset.scene)]; openInspector(state.stage + ' · 场景', r[0], r[2], r[3], { source: '小红书', author: null, date: null }); return; }
    if (el.dataset.save) {
      const i = Number(el.dataset.save);
      state.saved.has(i) ? state.saved.delete(i) : state.saved.add(i);
      renderMaterials(); toast(state.saved.has(i) ? '已保留为备选（演示，未写入）' : '已取消保留'); return;
    }
    if (el.dataset.bin) { toast('将按 ' + el.dataset.bin + ' 赞筛选材料（演示）'); return; }
    if (el.dataset.candidate) { toast('打开方向详情：' + el.dataset.candidate); return; }
    switch (el.id) {
      case 'treeClear': $('treeSearch').value = ''; renderTree(); $('treeSearch').focus(); break;
      case 'inspClose': case 'inspCancel': closeInspector(); break;
      case 'inspLocate': toast('将定位到原始材料（演示）'); break;
      case 'matrixButton': toast('打开主题 × 旅程矩阵（演示）'); break;
      case 'ownSetup': toast('前往我方账号设置（演示）'); break;
    }
  });
  $('scrim').addEventListener('click', closeInspector);
  document.addEventListener('keydown', e => { if (e.key === 'Escape' && $('inspector').classList.contains('open')) closeInspector(); });
  $('treeSearch').addEventListener('input', renderTree);
  $('materialSearch').addEventListener('input', renderMaterials);
  $('platformFilter').addEventListener('change', e => toast('统计平台：' + e.target.value + '（演示）'));
  $('timeFilter').addEventListener('change', e => toast('发布时间：' + e.target.value + '（演示）'));

  hydrateIcons();
  selectTopic('全部方向');
  window.__tm = { selectTopic, setView, state, renderJourney, openInspector: () => document.querySelector('[data-open="2"]').click() };
})();
