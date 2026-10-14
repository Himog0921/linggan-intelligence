// OPS-MIGRATION-001 D3 · 运营总览（PAGE-OPS-002）。除「设定目标」外不发写请求。
// 所有来自数据的文字都经 textContent 写入；未知一律显示「—」，不显示 0；结论句只由接口给的数字拼出。
'use strict';

const OverviewText = (() => {
  const missing = '—';
  const metricLabel = { published: '发布', links_opened: '链接打开', followers_gained: '加微' };
  const platformLabel = { xhs: '小红书', douyin: '抖音', both: '双平台' };
  const sliceTitle = { yesterday: '昨日加微', week: '本周加微', month: '本月加微', custom: '当期加微' };
  const riskLabel = { high: '待复盘', medium: '待确认' };
  const nextStep = {
    drop: '查看排行并复盘下降原因',
    zero_report: '确认是否漏填（切换前在工作台补）',
    pending_views: '补阅读量（切换前在工作台填）',
    owner_mismatch: '确认账号负责人和发布负责人',
  };
  function number(value) {
    if (value === null || value === undefined) return missing;
    return Number(value).toLocaleString('zh-CN');
  }
  function pct(value) {
    if (value === null || value === undefined) return missing;
    return `${Number(value).toFixed(1).replace(/\.0$/, '')}%`;
  }
  /** 环比：上期为 0 或未知时不比；0 写「持平」。 */
  function delta(value) {
    if (value === null || value === undefined) return { text: '环比 —', tone: '' };
    if (value === 0) return { text: '持平上期', tone: '' };
    const arrow = value > 0 ? '↑' : '↓';
    return { text: `${arrow}${pct(Math.abs(value))}`, tone: value > 0 ? 'up' : 'down' };
  }
  function insight(item) {
    if (item.kind === 'pace') {
      if (item.pace === 'achieved') return { tone: 'ok', text: `目标已达成（完成 ${pct(item.achievedPct)}）` };
      const known = typeof item.projectedPct === 'number';
      const projected = known ? `${item.projectedPct}%` : missing;
      if (item.pace === 'behind') return { tone: 'risk', text: `目标有缺口：按当前节奏预计完成 ${projected}` };
      // 预计完不成时不说「节奏正常」（Mog 2026-10-14），只陈述预计完成多少。
      if (known && item.projectedPct >= 100) return { tone: 'ok', text: `节奏正常：按当前节奏预计完成 ${projected}` };
      return { tone: '', text: `按当前节奏预计完成 ${projected}` };
    }
    if (item.kind === 'drag') {
      return { tone: 'watch', text: `${metricLabel[item.metric] || item.metric}下滑是主因（环比 −${pct(Math.abs(item.deltaPct))}）` };
    }
    return { tone: 'risk', text: `${item.names.join('、')}需今天复盘` };
  }
  function attention(item) {
    if (item.kind === 'drop') {
      return {
        text: `${item.operator} · ${metricLabel[item.metric] || item.metric}比上期下降 ${pct(Math.abs(item.dropPct))}（${number(item.previousValue)} → ${number(item.currentValue)}）`,
        action: nextStep.drop,
      };
    }
    if (item.kind === 'zero_report') {
      const metrics = item.zeroMetrics.map((zero) => metricLabel[zero.metric] || zero.metric).join('、');
      const previous = item.zeroMetrics.map((zero) => number(zero.previousValue)).join('、');
      return { text: `${item.operator} · ${metrics}本期 ${number(item.count)} 天都记的是 0（上期 ${previous}）`, action: nextStep.zero_report };
    }
    if (item.kind === 'pending_views') {
      const oldest = item.oldestDate ? `，最早 ${item.oldestDate}` : '';
      return { text: `${item.operator} · 待补阅读量 ${number(item.count)} 篇${oldest}`, action: nextStep.pending_views };
    }
    return {
      text: `账号「${item.account}」由 ${item.accountOperator} 负责，发布人是 ${item.operator}（${number(item.count)} 条）`,
      action: nextStep.owner_mismatch,
    };
  }
  /** 今日必须处理折叠部分的开关文字。 */
  function moreLabel(count, open) {
    return open ? '收起' : `另有 ${count} 项，展开`;
  }
  /** 排行里的「下一步」：下降写明是哪一项，其余用短句（完整建议在今日必须处理里）。 */
  function rankingStep(row) {
    if (row.riskKind === 'drop') return `复盘${metricLabel[row.riskMetric] || ''}下降`;
    if (row.riskKind === 'zero_report') return '确认是否漏填';
    if (row.riskKind === 'pending_views') return '补阅读量';
    if (row.riskKind === 'owner_mismatch') return '确认负责人';
    return '';
  }
  /** 柱高：最大值撑满，未知不画。 */
  function scale(values, height) {
    const max = Math.max(0, ...values.filter((value) => typeof value === 'number'));
    return (value) => (typeof value !== 'number' || max === 0 ? 0 : (value / max) * height);
  }
  /** 一段日期：只有一天时只写那一天。 */
  function span(range) {
    return range.from === range.to ? range.from : `${range.from} ~ ${range.to}`;
  }
  /** 时间段一行：日历上的本期、实际按哪几天算、比的是哪段上期。 */
  function windowLine(win, asOf) {
    const range = span(win.current);
    const through = asOf ? `数据截至 ${asOf.exportedAtBeijing.slice(5)}，按 ${win.completeThrough.slice(5)} 及以前计算` : '还没有迁入数据';
    if (!win.covered) return `${range} · 这段时间还没有迁入的数据（${through}）`;
    const counted = win.covered.to === win.current.to ? `${win.current.days} 天` : `按已迁入的 ${span(win.covered)}（${win.covered.days} 天）计算`;
    return `${range}（${counted}）· 比上期 ${span(win.previous)}`;
  }
  function blockEmptyText(win, emptyText) {
    return win.covered === null ? '这段时间还没有迁入的数据' : emptyText;
  }
  function trendPointNote(point) {
    if (point.coverage === 'not_migrated') return '还没迁入';
    return point.coverage === 'partial' ? `按 ${point.coveredTo} 及以前计算` : '';
  }
  return { missing, metricLabel, platformLabel, sliceTitle, riskLabel, nextStep, number, pct, delta, insight, attention, moreLabel, rankingStep, scale, span, windowLine, blockEmptyText, trendPointNote };
})();

if (typeof module !== 'undefined') module.exports = { OverviewText };

(() => {
  if (typeof document === 'undefined' || document.body.dataset.opsPage !== 'overview') return;
  const T = OverviewText;
  const business = document.body.dataset.opsBusiness || 'all';
  const params = new URLSearchParams(location.search);
  const status = document.getElementById('ops-status');
  const svgNs = 'http://www.w3.org/2000/svg';
  const state = { data: null, trend: null, granularity: 'day', metric: 'published', person: '' };
  const reasons = {
    ops_read_unavailable: '数据库暂时读不到',
    ops_business_not_found: '这个业务不存在',
    ops_parameter_invalid: '参数无效',
    read_model_not_connected: '本机服务没有连上数据库',
    ops_target_request_conflict: '这次保存的内容和之前的不一致',
  };

  function el(tag, props, children) {
    const node = document.createElement(tag);
    Object.entries(props || {}).forEach(([key, value]) => {
      if (value === undefined || value === null || value === false) return;
      if (key === 'text') node.textContent = value;
      else if (key === 'className') node.className = value;
      else if (key === 'dataset') Object.assign(node.dataset, value);
      else node.setAttribute(key, value === true ? '' : value);
    });
    (children || []).forEach((child) => child && node.append(child));
    return node;
  }
  function svg(tag, attrs, children) {
    const node = document.createElementNS(svgNs, tag);
    Object.entries(attrs || {}).forEach(([key, value]) => node.setAttribute(key, String(value)));
    (children || []).forEach((child) => child && node.append(child));
    return node;
  }
  function titled(node, text) {
    node.append(svg('title', {}, [document.createTextNode(text)]));
    return node;
  }
  async function request(url, options) {
    let response;
    try {
      response = await fetch(url, { credentials: 'same-origin', headers: { accept: 'application/json', ...(options && options.headers) }, ...options });
    } catch {
      throw new Error('连不上本机服务');
    }
    let body = null;
    try { body = await response.json(); } catch { /* 下面按状态码说明 */ }
    if (!response.ok) throw new Error(reasons[body && body.code] || `本机服务返回 ${response.status}`);
    if (!body) throw new Error('返回的内容无法读取');
    return body;
  }
  function setStatus(text, retry) {
    status.textContent = text || '';
    status.dataset.error = text ? 'true' : 'false';
    if (retry) {
      const button = el('button', { type: 'button', text: '重试' });
      button.addEventListener('click', retry);
      status.append(' ', button);
    }
  }
  function field(name) {
    return document.querySelector(`[data-field="${name}"]`);
  }
  function setDelta(node, value) {
    const delta = T.delta(value);
    node.textContent = delta.text;
    if (delta.tone) node.dataset.tone = delta.tone; else delete node.dataset.tone;
  }
  function apiQuery(extra) {
    const query = new URLSearchParams({ domain: business, ...extra });
    [...query.keys()].forEach((key) => { if (!query.get(key)) query.delete(key); });
    return query;
  }

  // ---------- ① 加微与目标 ----------
  function renderHero(data) {
    const { kpi, window: win } = data;
    document.getElementById('ops-hero-title').textContent = T.sliceTitle[win.slice];
    const followers = field('followers');
    followers.textContent = T.number(kpi.current.followersGained);
    if (kpi.current.followersGained === null) followers.dataset.state = 'unknown'; else delete followers.dataset.state;
    followers.title = kpi.current.followersGained === null ? '这段时间没有迁入的每日数据：未知，不是 0' : '';
    setDelta(field('followers-delta'), kpi.delta.followersGainedPct);
    field('followers-previous').textContent = win.previous
      ? `上期（${T.span(win.previous)}）实际 ${T.number(kpi.previous.followersGained)}`
      : '这段时间还没有迁入的数据，不比上期';
    const list = document.getElementById('ops-insights');
    list.replaceChildren(...data.insights.map((item) => {
      const sentence = T.insight(item);
      return el('li', { text: sentence.text, dataset: { tone: sentence.tone } });
    }));
    renderTarget(data);
  }
  function renderTarget(data) {
    const { target } = data;
    const box = document.getElementById('ops-target');
    if (target.state === 'not_applicable') {
      box.replaceChildren(
        el('div', { className: 'ops-target-head' }, [el('span', { text: '加微目标' }), el('b', { text: '自定义时段不对应目标', dataset: { state: 'unknown' } })]),
        el('p', { className: 'ops-note-line', text: '目标按周、按月设；切到本周或本月可以查看和设定。' }),
      );
      return;
    }
    const period = target.periodType === 'month' ? '本月目标' : '本周目标';
    const open = el('button', { type: 'button', text: target.state === 'set' ? '修改目标' : '设定目标' });
    open.addEventListener('click', () => openTargetDialog(target.periodType));
    if (target.state === 'not_set') {
      box.replaceChildren(
        el('div', { className: 'ops-target-head' }, [el('span', { text: `${period}（${target.periodKey}）` }), el('b', { text: '未设定', dataset: { state: 'unknown' } })]),
        el('p', { className: 'ops-note-line', text: '没设目标时不估算、不判断节奏。' }),
        el('div', { className: 'ops-target-foot' }, [el('span', { text: '只能在本机设定' }), open]),
      );
      return;
    }
    const revision = target.revision;
    const bar = el('div', { className: 'ops-progress', role: 'img', 'aria-label': `完成 ${T.pct(target.achievedPct)}，时间进度 ${T.pct(target.timeProgressPct)}` }, [el('i'), el('b')]);
    bar.style.setProperty('--ops-fill', `${Math.min(target.achievedPct || 0, 100)}%`);
    bar.style.setProperty('--ops-time', `${Math.min(target.timeProgressPct || 0, 100)}%`);
    const pair = (label, value) => el('div', {}, [el('dt', { text: label }), el('dd', { text: value })]);
    const known = target.periodFollowers !== null && target.elapsedDays !== null;
    box.replaceChildren(
      el('div', { className: 'ops-target-head' }, [el('span', { text: `${period}（${target.periodKey}）` }), el('b', { text: T.number(revision.targetValue) })]),
      bar,
      el('dl', { className: 'ops-target-grid' }, [
        pair('本期已完成', known ? `${T.number(target.periodFollowers)}（${T.pct(target.achievedPct)}）` : '还没有迁入的数据'),
        pair('时间进度', known ? `${T.pct(target.timeProgressPct)}（按已迁入 ${target.elapsedDays} 天）` : T.missing),
        pair('每日还需', target.dailyNeeded === null ? T.missing : T.number(target.dailyNeeded)),
        pair('预计周期末', target.projectedEnd === null ? T.missing : T.number(target.projectedEnd)),
      ]),
      el('div', { className: 'ops-target-foot' }, [el('span', { text: `${revision.setAtBeijing} 改为 ${T.number(revision.targetValue)} · 本机` }), open]),
    );
    if (target.pace === 'achieved') box.querySelector('.ops-target-head span').textContent += ' · 目标已达成';
  }

  // ---------- ② 漏斗 ----------
  function renderFunnel(data) {
    const { kpi, funnel } = data;
    const stage = (label, value, deltaPct, note, unknown) => {
      const watch = typeof deltaPct === 'number' && deltaPct <= -20;
      const delta = T.delta(deltaPct);
      return el('li', { dataset: { watch: String(watch) } }, [
        el('span', { text: watch ? `${label} · 观察中` : label }),
        el('b', { text: value, dataset: unknown ? { state: 'unknown' } : undefined }),
        el('small', { text: note || delta.text }),
      ]);
    };
    const openRate = funnel.publishToOpenAvailable ? `发布 → 打开 ${T.pct(funnel.current.publishToOpen)}` : '发布 → 打开：选了业务时不计算（打开不分业务）';
    document.getElementById('ops-funnel').replaceChildren(
      stage('发布', T.number(kpi.current.published), kpi.delta.publishedPct),
      stage('有效发布', '未接入', null, '自动发现我方帖子上线后接入', true),
      stage('链接打开', T.number(kpi.current.linksOpened), kpi.delta.linksOpenedPct, `${openRate} · ${T.delta(kpi.delta.linksOpenedPct).text}`),
      stage('加微', T.number(kpi.current.followersGained), kpi.delta.followersGainedPct, `打开 → 加微 ${T.pct(funnel.current.openToFollow)} · ${T.delta(kpi.delta.followersGainedPct).text}`),
    );
    field('conversion').textContent = T.pct(kpi.current.conversionRate);
    field('hits').textContent = T.number(kpi.current.hits);
    setDelta(field('hits-delta'), kpi.delta.hitsPct);
    const watched = [kpi.delta.publishedPct, kpi.delta.linksOpenedPct, kpi.delta.followersGainedPct].some((value) => typeof value === 'number' && value <= -20);
    field('funnel-note').textContent = watched ? '标「观察中」的环节比上期下降 20% 以上。' : '各环节转化平稳，无明显掉点。';
  }

  // ---------- ③ 趋势、每日产出、今日必须处理 ----------
  // 图按容器的实际宽度画（1 个单位 = 1 像素），窄屏上坐标文字不会跟着缩小。
  function chartWidth(box) {
    return Math.max(Math.floor(box.clientWidth) || 640, 240);
  }
  /** 坐标文字以柱子中心对齐；靠边的改为左对齐或右对齐，不被裁掉。 */
  function axisLabel(center, y, value, width) {
    const half = value.length * 3.5;
    const anchor = center - half < 0 ? 'start' : center + half > width ? 'end' : 'middle';
    const x = anchor === 'start' ? 0 : anchor === 'end' ? width : center;
    return svg('text', { x, y, 'text-anchor': anchor, class: 'ops-axis' }, [document.createTextNode(value)]);
  }
  function bars(container, columns, options) {
    const width = chartWidth(container);
    const height = options.height;
    const top = 8;
    const slot = width / Math.max(columns.length, 1);
    const chart = svg('svg', { viewBox: `0 0 ${width} ${height + 24}` });
    chart.append(svg('line', { x1: 0, x2: width, y1: height + top, y2: height + top, class: 'ops-grid' }));
    columns.forEach((column, index) => {
      const x = index * slot + slot * 0.15;
      let base = height + top;
      column.parts.forEach((part) => {
        if (part.height <= 0) return;
        base -= part.height;
        chart.append(titled(svg('rect', { x, y: base, width: slot * 0.7, height: part.height, class: part.className }), part.title));
      });
      if (column.unknown) chart.append(titled(svg('rect', { x, y: height + top - 2, width: slot * 0.7, height: 2, class: 'ops-gap' }), column.unknown));
      if (column.label) chart.append(axisLabel(x + slot * 0.35, height + top + 14, column.label, width));
    });
    container.replaceChildren(chart);
  }
  function labelEvery(count) {
    return Math.max(1, Math.ceil(count / 8));
  }
  /** 上排加微柱，下排爆款柱，各按自己的最大值缩放，不相加。 */
  function trendChart(box, points) {
    const width = chartWidth(box);
    const followTop = 18;
    const followHeight = 92;
    const hitTop = followTop + followHeight + 22;
    const hitHeight = 22;
    const slot = width / points.length;
    const follow = T.scale(points.map((point) => point.followersGained), followHeight);
    const hit = T.scale(points.map((point) => point.hits), hitHeight);
    const every = labelEvery(points.length);
    const text = (x, y, value, anchor) => svg('text', { x, y, 'text-anchor': anchor || 'start', class: 'ops-axis' }, [document.createTextNode(value)]);
    const chart = svg('svg', { viewBox: `0 0 ${width} ${hitTop + hitHeight + 18}` });
    chart.append(
      text(0, 10, '加微'),
      text(0, hitTop - 6, '爆款'),
      svg('line', { x1: 0, x2: width, y1: followTop + followHeight, y2: followTop + followHeight, class: 'ops-grid' }),
      svg('line', { x1: 0, x2: width, y1: hitTop + hitHeight, y2: hitTop + hitHeight, class: 'ops-grid' }),
    );
    points.forEach((point, index) => {
      const suffix = point.coverage === 'partial' ? ` · ${T.trendPointNote(point)}` : '';
      const x = index * slot + slot * 0.15;
      const w = slot * 0.7;
      const f = follow(point.followersGained);
      if (point.followersGained === null) {
        chart.append(titled(svg('rect', { x, y: followTop + followHeight - 2, width: w, height: 2, class: 'ops-gap' }), `${point.label} 没有迁入的每日数据${suffix}`));
      } else if (f > 0) {
        chart.append(titled(svg('rect', { x, y: followTop + followHeight - f, width: w, height: f, class: 'ops-followers' }), `${point.label} 加微 ${T.number(point.followersGained)}${suffix}`));
      }
      if (point.hits === null) {
        chart.append(titled(svg('rect', { x, y: hitTop + hitHeight - 2, width: w, height: 2, class: 'ops-gap' }), `${point.label} 爆款：还没迁入`));
      } else if (point.hits > 0) {
        const h = Math.max(hit(point.hits), 3);
        chart.append(titled(svg('rect', { x, y: hitTop + hitHeight - h, width: w, height: h, class: 'ops-hits' }), `${point.label} 爆款 ${point.hits} 篇${suffix}`));
      }
      if (index % every === 0) chart.append(axisLabel(x + w / 2, hitTop + hitHeight + 14, point.label, width));
    });
    const partialNotes = points.filter((point) => point.coverage === 'partial').map((point) => el('p', { className: 'ops-note-line', text: `${point.label} · ${T.trendPointNote(point)}` }));
    box.replaceChildren(chart, ...partialNotes);
  }
  let latestTrend = 0;
  async function loadTrend() {
    const box = document.getElementById('ops-trend');
    const requestNo = ++latestTrend;
    try {
      const data = await request(`/api/local/ops/overview/trend?${apiQuery({ granularity: state.granularity })}`);
      if (requestNo !== latestTrend) return;
      state.trend = data.points.length ? data.points : null;
      if (!data.points.length) {
        box.replaceChildren(el('p', { className: 'ops-chart-empty', text: '暂无趋势数据' }));
        return;
      }
      trendChart(box, data.points);
      box.setAttribute('aria-label', `加微与爆款长趋势，${data.points.length} 个${{ day: '日', week: '周', month: '月' }[state.granularity]}`);
    } catch (error) {
      if (requestNo !== latestTrend) return;
      state.trend = null;
      box.replaceChildren(el('p', { className: 'ops-chart-empty', text: `趋势读取失败（${error.message}），可以重新点粒度重试` }));
    }
  }
  function renderOutputTabs(data) {
    const nav = document.getElementById('ops-output-people');
    const names = ['', ...data.dailyOutput.operators];
    if (state.person && !names.includes(state.person)) state.person = '';
    nav.replaceChildren(...names.map((name) => {
      const button = el('button', { type: 'button', text: name || '全部', 'aria-pressed': String(state.person === name) });
      button.addEventListener('click', () => { state.person = name; renderOutput(); });
      return button;
    }));
  }
  function renderOutput() {
    const data = state.data;
    if (!data) return;
    renderOutputTabs(data);
    document.querySelectorAll('[data-metric]').forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.metric === state.metric)));
    const output = data.dailyOutput;
    const mine = (row) => !state.person || row.operator === state.person;
    const migrated = (day) => output.coveredTo !== null && day <= output.coveredTo;
    const every = labelEvery(output.days.length);
    const box = document.getElementById('ops-daily-output');
    const legend = document.getElementById('ops-output-legend');
    if (state.metric === 'published') {
      const totals = output.days.map((day) => output.published.filter((row) => row.date === day && mine(row))
        .reduce((sum, row) => ({ xhs: sum.xhs + row.xhs, douyin: sum.douyin + row.douyin }), { xhs: 0, douyin: 0 }));
      const height = T.scale(totals.map((total) => total.xhs + total.douyin), 120);
      if (output.coveredTo === null) {
        box.replaceChildren(el('p', { className: 'ops-chart-empty', text: '这段时间还没有迁入的数据' }));
      } else if (!totals.some((total) => total.xhs + total.douyin > 0)) {
        box.replaceChildren(el('p', { className: 'ops-chart-empty', text: '当期无发布' }));
      } else {
        bars(box, output.days.map((day, index) => ({
          label: index % every === 0 ? day.slice(5) : '',
          unknown: migrated(day) ? '' : `${day} 还没有迁入`,
          parts: [
            { height: height(totals[index].xhs), className: 'ops-xhs', title: `${day} 小红书 ${totals[index].xhs} 篇` },
            { height: height(totals[index].douyin), className: 'ops-douyin', title: `${day} 抖音 ${totals[index].douyin} 篇` },
          ],
        })), { height: 120 });
      }
      legend.replaceChildren(el('span', {}, [el('i', { className: 'ops-xhs' }), '小红书']), el('span', {}, [el('i', { className: 'ops-douyin' }), '抖音']));
    } else {
      const values = output.days.map((day) => {
        const rows = output.followers.filter((row) => row.date === day && mine(row));
        return rows.length ? rows.reduce((sum, row) => sum + row.value, 0) : null;
      });
      const height = T.scale(values, 120);
      bars(box, output.days.map((day, index) => ({
        label: index % every === 0 ? day.slice(5) : '',
        unknown: !migrated(day) ? `${day} 还没有迁入` : values[index] === null ? `${day} 没有每日数据` : '',
        parts: [{ height: height(values[index]), className: 'ops-followers', title: `${day} 加微 ${T.number(values[index])}` }],
      })), { height: 120 });
      legend.replaceChildren(el('span', {}, [el('i', { className: 'ops-followers' }), '加微（按人记，不分平台）']));
    }
  }
  function renderAttention(data) {
    const list = document.getElementById('ops-attention');
    const more = document.getElementById('ops-attention-more');
    const { attention } = data;
    more.replaceChildren();
    if (!attention.items.length) {
      list.replaceChildren(el('li', { className: 'ops-attention-empty', text: '各项指标在正常波动范围内' }));
      return;
    }
    // 前 5 项直接显示，其余折叠在同一个列表里，点开在原处显示（Mog 2026-10-14）。
    const row = (item, folded) => {
      const sentence = T.attention(item);
      return el('li', { hidden: folded, dataset: folded ? { severity: item.severity, folded: 'true' } : { severity: item.severity } }, [el('i', { 'aria-hidden': 'true' }), el('b', { text: sentence.text }), el('small', { text: sentence.action })]);
    };
    list.replaceChildren(...attention.items.map((item) => row(item, false)), ...attention.rest.map((item) => row(item, true)));
    if (!attention.rest.length) return;
    const toggle = el('button', { type: 'button', 'aria-expanded': 'false', 'aria-controls': 'ops-attention', text: T.moreLabel(attention.rest.length, false) });
    toggle.addEventListener('click', () => {
      const open = toggle.getAttribute('aria-expanded') !== 'true';
      list.querySelectorAll('li[data-folded]').forEach((li) => { li.hidden = !open; });
      toggle.setAttribute('aria-expanded', String(open));
      toggle.textContent = T.moreLabel(attention.rest.length, open);
    });
    more.append(toggle);
  }

  // ---------- ④ 排行 ----------
  function sparkline(values) {
    const width = 84;
    const height = 20;
    const max = Math.max(1, ...values.filter((value) => typeof value === 'number'));
    const chart = svg('svg', { viewBox: `0 0 ${width} ${height}`, class: 'ops-spark', role: 'img', 'aria-label': `近 7 天加微 ${values.map((value) => T.number(value)).join('、')}` });
    let segment = [];
    const flush = () => {
      if (segment.length > 1) chart.append(svg('polyline', { points: segment.join(' ') }));
      segment = [];
    };
    values.forEach((value, index) => {
      if (typeof value !== 'number') { flush(); return; }
      segment.push(`${(index / 6) * (width - 2) + 1},${height - 1 - (value / max) * (height - 2)}`);
    });
    flush();
    return chart;
  }
  function renderRanking(data) {
    const body = document.getElementById('ops-ranking');
    if (!data.ranking.length) {
      body.replaceChildren(el('tr', {}, [el('td', { className: 'ops-empty', colspan: '13', text: T.blockEmptyText(data.window, '当期没有运营数据') })]));
      return;
    }
    const cell = (text, className, title) => el('td', { className, text, title });
    body.replaceChildren(...data.ranking.map((row) => el('tr', {}, [
      cell(row.operator),
      cell(row.platform ? T.platformLabel[row.platform] : T.missing),
      el('td', {}, [sparkline(row.sparkline)]),
      cell(T.number(row.responsibleAccounts), 'ops-number'),
      cell(T.number(row.published), 'ops-number'),
      cell('未接入', 'ops-number ops-missing', '自动发现我方帖子上线后接入'),
      cell(T.number(row.linksOpened), 'ops-number'),
      cell(T.number(row.followersGained), 'ops-number'),
      cell(T.pct(row.conversionRate), 'ops-number'),
      cell(T.number(row.hits), 'ops-number'),
      cell(T.number(row.pendingViews), 'ops-number', '发出满 7 天还没填阅读量的小红书帖子'),
      cell(row.risk ? T.riskLabel[row.risk] : ''),
      cell(T.rankingStep(row)),
    ])));
  }

  // ---------- ⑤ 平台占比、矩阵 ----------
  function renderShare(data) {
    const box = document.getElementById('ops-platform-share');
    if (data.window.covered === null) {
      box.replaceChildren(el('p', { className: 'ops-chart-empty', text: T.blockEmptyText(data.window, '当期无发布') }));
      return;
    }
    const { xhs, douyin } = data.platformShare;
    const total = xhs + douyin;
    if (!total) {
      box.replaceChildren(el('p', { className: 'ops-chart-empty', text: '当期无发布' }));
      return;
    }
    const segment = (count, className) => {
      const node = el('span', { className, title: `${count} 篇` });
      node.style.flex = String(count);
      return node;
    };
    const label = (name, count, className) => el('span', {}, [el('i', { className }), `${name} ${T.number(count)} 篇（${T.pct((count / total) * 100)}）`]);
    box.replaceChildren(
      el('div', { className: 'ops-share', role: 'img', 'aria-label': `小红书 ${xhs} 篇，抖音 ${douyin} 篇` }, [segment(xhs, 'ops-xhs'), segment(douyin, 'ops-douyin')]),
      el('p', { className: 'ops-share-labels' }, [label('小红书', xhs, 'ops-xhs'), label('抖音', douyin, 'ops-douyin')]),
    );
  }
  function renderMatrix(data) {
    const { matrix } = data;
    const box = document.getElementById('ops-matrix');
    if (!matrix.cells.length) {
      box.replaceChildren(el('p', { className: 'ops-chart-empty', text: T.blockEmptyText(data.window, '当期无矩阵数据') }));
      return;
    }
    const max = Math.max(...matrix.cells.map((cell) => cell.published));
    const head = el('tr', {}, [el('th', { scope: 'col', text: '账号' }), el('th', { scope: 'col', text: '平台' }), ...matrix.operators.map((name) => el('th', { scope: 'col', className: 'ops-number', text: name }))]);
    const rows = matrix.accounts.map((account) => {
      const name = el('th', { scope: 'row' }, [el('span', { className: 'ops-matrix-name', text: account.name, title: account.name })]);
      if (account.retired) name.append(' ', el('span', { className: 'ops-tag', text: '已停用', title: '这个账号已在工作台删除' }));
      return el('tr', {}, [name, el('td', { text: T.platformLabel[account.platform] || account.platform }), ...matrix.operators.map((operator) => {
        const cell = matrix.cells.find((item) => item.account === account.ref && item.operator === operator);
        if (!cell) return el('td', { className: 'ops-matrix-empty', text: '·' });
        const node = el('td', { dataset: { heat: '' }, text: cell.hits ? `${cell.published}（${cell.hits}）` : String(cell.published), title: `${operator} 在 ${account.name} 发布 ${cell.published} 篇，爆款 ${cell.hits} 篇` });
        node.style.setProperty('--ops-heat', String(cell.published / max));
        return node;
      })]);
    });
    const table = el('table', { className: 'ops-table ops-matrix' }, [el('thead', {}, [head]), el('tbody', {}, rows)]);
    box.classList.add('ops-matrix-wrap');
    box.replaceChildren(table);
  }

  // ---------- 读取与时间段 ----------
  function sliceQuery() {
    const slice = params.get('slice') || 'week';
    return slice === 'custom' ? { slice, from: params.get('from') || '', to: params.get('to') || '' } : { slice };
  }
  function syncUrl() {
    params.set('domain', business);
    history.replaceState(null, '', `${location.pathname}?${params}`);
  }
  let latestLoad = 0;
  // 读取成功后要显示的一句话（如「目标没有变化。」）；读取本身会清掉旧提示。
  let noteAfterLoad = '';
  async function load() {
    const wrap = document.querySelector('.ops-overview');
    const requestNo = ++latestLoad;
    wrap.setAttribute('aria-busy', 'true');
    document.getElementById('ops-hero-title').textContent = T.sliceTitle[params.get('slice') || 'week'] || T.sliceTitle.week;
    document.querySelectorAll('[data-slice]').forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.slice === (params.get('slice') || 'week'))));
    try {
      const data = await request(`/api/local/ops/overview?${apiQuery(sliceQuery())}`);
      // 只让最新一次请求落到页面：慢回来的旧时间段不能盖住新选的时间段。
      if (requestNo !== latestLoad) return;
      state.data = data;
      wrap.hidden = false;
      document.getElementById('ops-window').textContent = T.windowLine(data.window, data.asOf);
      document.getElementById('ops-scope-note').hidden = !data.businessSelected;
      renderHero(data);
      renderFunnel(data);
      renderAttention(data);
      renderRanking(data);
      renderShare(data);
      renderMatrix(data);
      renderOutput();
      setStatus(data.asOf ? noteAfterLoad : '还没有迁入数据。');
      noteAfterLoad = '';
    } catch (error) {
      if (requestNo === latestLoad) {
        // 页签已经换成新时间段：旧时间段的数字不能留在页面上冒充新时间段。
        state.data = null;
        wrap.hidden = true;
        document.getElementById('ops-window').textContent = '';
        setStatus(`总览读取失败（${error.message}），可以重试。`, load);
      }
    } finally {
      if (requestNo === latestLoad) wrap.setAttribute('aria-busy', 'false');
    }
  }

  // ---------- 设定目标 ----------
  const dialog = document.getElementById('ops-target-dialog');
  const form = document.getElementById('ops-target-form');
  // current 为 undefined 表示还没读到当前目标，这时不允许保存。
  const target = { current: undefined, requestId: null, periodType: 'week' };
  function periodKey(periodType) {
    return periodType === 'month' ? state.data.window.thisMonth : state.data.window.thisWeek;
  }
  function readTargetHistory(periodType) {
    return request(`/api/local/ops/targets?${new URLSearchParams({ periodType, periodKey: periodKey(periodType) })}`);
  }
  function showTargetHistory(history) {
    target.current = history.current;
    field('target-current').textContent = history.current ? `当前：${T.number(history.current.targetValue)}（${history.current.setAtBeijing} · 本机）` : '当前：未设定';
    field('target-history').replaceChildren(...(history.revisions.length
      ? history.revisions.map((revision) => el('li', { text: `${revision.setAtBeijing} 改为 ${T.number(revision.targetValue)} · 本机` }))
      : [el('li', { text: '还没有修改记录' })]));
  }
  // 只让最后一次读取落到对话框：切换周期时，慢回来的另一个周期的记录不能盖住当前周期。
  let latestTargetRead = 0;
  async function loadTargetHistory() {
    const periodType = form.elements.periodType.value;
    const readNo = ++latestTargetRead;
    target.periodType = periodType;
    target.requestId = null;
    target.current = undefined;
    field('target-error').textContent = '';
    field('target-current').textContent = '当前：读取中';
    try {
      const history = await readTargetHistory(periodType);
      if (readNo !== latestTargetRead) return;
      showTargetHistory(history);
      form.elements.targetValue.value = history.current ? history.current.targetValue : '';
    } catch (error) {
      if (readNo !== latestTargetRead) return;
      target.current = undefined;
      field('target-current').textContent = `当前目标读取失败（${error.message}），请关闭后重试`;
    }
  }
  function openTargetDialog(periodType) {
    if (!state.data) return;
    form.elements.periodType.value = periodType || 'week';
    field('week-label').textContent = `本周（${state.data.window.thisWeek}）`;
    field('month-label').textContent = `本月（${state.data.window.thisMonth}）`;
    form.elements.targetValue.value = '';
    dialog.showModal();
    loadTargetHistory();
  }
  form.addEventListener('change', (event) => { if (event.target.name === 'periodType') loadTargetHistory(); });
  // 改了目标值就是一次新的保存，换新的请求号；只有原样重试才沿用旧请求号。
  form.elements.targetValue.addEventListener('input', () => { target.requestId = null; });
  document.getElementById('ops-target-cancel').addEventListener('click', () => dialog.close());
  form.addEventListener('submit', async (event) => {
    event.preventDefault();
    const value = Number(form.elements.targetValue.value);
    if (!Number.isInteger(value) || value < 1 || value > 1000000) {
      field('target-error').textContent = '请输入 1 到 1000000 之间的整数';
      return;
    }
    if (target.current === undefined) {
      field('target-error').textContent = '当前目标还没读到，不能保存';
      return;
    }
    // 同一次保存重试时沿用同一个请求号，服务端只生效一次。
    target.requestId = target.requestId || crypto.randomUUID();
    const periodType = target.periodType;
    const save = document.getElementById('ops-target-save');
    save.disabled = true;
    try {
      const receipt = await request('/api/local/ops/targets', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          periodType,
          periodKey: periodKey(periodType),
          targetValue: value,
          expectedRevision: target.current ? target.current.revisionRef : null,
          requestId: target.requestId,
        }),
      });
      // 保存途中切换了周期：这份回执属于另一个周期，不落到对话框上。
      if (periodType !== target.periodType) return;
      target.requestId = null;
      if (receipt.outcome === 'stale') {
        showTargetHistory({ current: receipt.current, revisions: [] });
        field('target-error').textContent = '目标刚被修改过，已显示最新值；确认后再保存';
        const readNo = ++latestTargetRead;
        try {
          const history = await readTargetHistory(target.periodType);
          if (readNo === latestTargetRead) showTargetHistory(history);
        } catch {
          if (readNo === latestTargetRead) field('target-history').replaceChildren(el('li', { text: '修改记录读取失败，关闭后重新打开可再读' }));
        }
        return;
      }
      dialog.close();
      noteAfterLoad = receipt.outcome === 'unchanged' ? '目标没有变化。' : '';
      load();
    } catch (error) {
      if (periodType === target.periodType) field('target-error').textContent = `保存失败（${error.message}），输入已保留，可以再试`;
    } finally {
      save.disabled = false;
    }
  });

  // ---------- 自定义时段 ----------
  const custom = document.getElementById('ops-custom-dialog');
  const customForm = document.getElementById('ops-custom-form');
  document.getElementById('ops-custom-cancel').addEventListener('click', () => custom.close());
  customForm.addEventListener('submit', (event) => {
    event.preventDefault();
    const from = customForm.elements.from.value;
    const to = customForm.elements.to.value;
    const days = (Date.parse(`${to}T00:00:00Z`) - Date.parse(`${from}T00:00:00Z`)) / 86400000 + 1;
    if (!from || !to || !(days >= 1 && days <= 366)) {
      field('custom-error').textContent = '开始日期不能晚于结束日期，最长 366 天';
      return;
    }
    custom.close();
    params.set('slice', 'custom');
    params.set('from', from);
    params.set('to', to);
    syncUrl();
    load();
  });
  document.querySelectorAll('[data-slice]').forEach((button) => button.addEventListener('click', () => {
    if (button.dataset.slice === 'custom') {
      field('custom-error').textContent = '';
      customForm.elements.from.value = params.get('from') || '';
      customForm.elements.to.value = params.get('to') || '';
      custom.showModal();
      return;
    }
    params.set('slice', button.dataset.slice);
    params.delete('from');
    params.delete('to');
    syncUrl();
    load();
  }));
  document.querySelectorAll('[data-granularity]').forEach((button) => button.addEventListener('click', () => {
    state.granularity = button.dataset.granularity;
    document.querySelectorAll('[data-granularity]').forEach((other) => other.setAttribute('aria-pressed', String(other === button)));
    loadTrend();
  }));
  document.querySelectorAll('[data-metric]').forEach((button) => button.addEventListener('click', () => {
    state.metric = button.dataset.metric;
    renderOutput();
  }));

  let drawnWidth = 0;
  new ResizeObserver(() => {
    const box = document.getElementById('ops-trend');
    if (!box.clientWidth || box.clientWidth === drawnWidth) return;
    drawnWidth = box.clientWidth;
    if (state.trend) trendChart(box, state.trend);
    renderOutput();
  }).observe(document.getElementById('ops-trend'));

  syncUrl();
  load();
  loadTrend();
})();
