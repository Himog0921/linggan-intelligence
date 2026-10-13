// OPS-MIGRATION-001 D2 · 运营页面（PAGE-OPS-001）。只读：本文件不发出任何写请求。
// 所有来自数据的文字都经 textContent 写入；未知一律显示「—」，不显示 0。
'use strict';

const OpsFormat = (() => {
  const missing = '—';
  /** 0 is a fact and stays 0; ≥10,000 abbreviates to 万 while the exact value stays reachable. */
  function count(value) {
    if (value === null || value === undefined) return { text: missing, title: '没有值或未知', missing: true };
    const exact = Number(value).toLocaleString('zh-CN');
    if (Math.abs(value) < 10000) return { text: exact, title: '' };
    const short = (value / 10000).toFixed(1).replace(/\.0$/, '');
    return { text: `${short}万`, title: exact };
  }
  function percent(value, digits) {
    if (value === null || value === undefined) return { text: missing, title: '没有值或未知', missing: true };
    return { text: `${Number(value).toFixed(digits).replace(/\.0+$/, '')}%`, title: '' };
  }
  function platform(value) {
    return value === 'douyin' ? '抖音' : '小红书';
  }
  const accountStatus = {
    active: '使用中', violation_disabled: '违规停用', warming: '养号中', paused: '暂停', archived: '已归档',
  };
  function safeHref(value) {
    try {
      const url = new URL(value);
      return url.protocol === 'http:' || url.protocol === 'https:' ? url.href : null;
    } catch {
      return null;
    }
  }
  return { missing, count, percent, platform, accountStatus, safeHref };
})();

if (typeof module !== 'undefined') module.exports = { OpsFormat };

(() => {
  if (typeof document === 'undefined') return;
  const page = document.body.dataset.opsPage;
  const business = document.body.dataset.opsBusiness || 'all';
  const params = new URLSearchParams(location.search);
  const status = document.getElementById('ops-status');
  const rows = document.getElementById('ops-rows');
  const wrap = document.querySelector('.ops-table-wrap');
  const readonly = '切换前只读：录入仍在内容工作台';

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
  const tag = (text, tone, title) => el('span', { className: 'ops-tag', text, title, dataset: tone ? { tone } : undefined });
  const missing = (title) => el('span', { className: 'ops-missing', text: OpsFormat.missing, title: title || '没有值或未知' });
  function numberCell(value) {
    const formatted = OpsFormat.count(value);
    return el('td', { className: 'ops-number' }, [
      formatted.missing ? missing() : el('span', { text: formatted.text, title: formatted.title || undefined }),
    ]);
  }
  function textCell(value, className) {
    return el('td', {}, [value ? el('span', { className: className || 'ops-ellipsis', text: value, title: value }) : missing()]);
  }

  function setStatus(text, isError, retry) {
    if (!status) return;
    status.textContent = text || '';
    status.dataset.error = isError ? 'true' : 'false';
    if (retry) status.append(' ', el('button', { type: 'button', text: '重试' }));
    const button = status.querySelector('button');
    if (button) button.addEventListener('click', retry);
  }
  function setBusy(flag) {
    if (wrap) wrap.setAttribute('aria-busy', String(flag));
  }
  function syncUrl() {
    params.set('domain', business);
    history.replaceState(null, '', `${location.pathname}?${params}`);
  }
  function apiUrl(path, extra) {
    const query = new URLSearchParams({ domain: business, ...extra });
    [...query.keys()].forEach((key) => { if (!query.get(key)) query.delete(key); });
    return `/api/local/ops/${path}?${query}`;
  }
  const readReasons = {
    ops_read_unavailable: '数据库暂时读不到',
    ops_business_not_found: '这个业务不存在',
    ops_parameter_invalid: '筛选条件无效',
    read_model_not_connected: '本机服务没有连上数据库',
  };
  async function getJson(url) {
    let response;
    try {
      response = await fetch(url, { headers: { accept: 'application/json' }, credentials: 'same-origin' });
    } catch {
      throw new Error('连不上本机服务');
    }
    if (!response.ok) {
      let code = '';
      try { code = (await response.json()).code || ''; } catch { /* keep the HTTP status */ }
      throw new Error(readReasons[code] || `本机服务返回 ${response.status}`);
    }
    try {
      return await response.json();
    } catch {
      throw new Error('返回的内容无法读取');
    }
  }
  // One status line serves the readouts and the table, so a retry reloads everything that failed
  // and a later success never hides an earlier failure.
  const failures = new Map();
  function showFailures() {
    if (!failures.size) {
      setStatus('');
      return;
    }
    const labels = [...failures.values()].map((failure) => failure.label).join('、');
    const reasons = [...new Set([...failures.values()].map((failure) => failure.reason))].join('；');
    setStatus(`${labels}读取失败（${reasons}），可以重试；筛选条件保持不变。`, true, () => {
      const loaders = [...failures.keys()];
      failures.clear();
      showFailures();
      loaders.forEach((loader) => loader());
    });
  }
  function failed(loader, label, error) {
    failures.set(loader, { label, reason: error.message });
    showFailures();
  }
  function succeeded(loader) {
    failures.delete(loader);
    showFailures();
  }
  // Only the newest table request may paint: a slow answer to an older filter must not land
  // under the tab the user has since moved to.
  let latestTableRequest = 0;
  function emptyRow(columns, text, action) {
    rows.replaceChildren(el('tr', {}, [el('td', { className: 'ops-empty', colspan: String(columns) }, [el('p', { text }), action])]));
  }
  function renderTabs(names, current, onSelect) {
    const nav = document.getElementById('ops-operators');
    if (!nav) return;
    const buttons = ['', ...names].map((name) => {
      const button = el('button', { type: 'button', text: name || '全部', 'aria-pressed': String((current || '') === name) });
      button.addEventListener('click', () => onSelect(name));
      return button;
    });
    nav.replaceChildren(...buttons);
  }
  function disabledActions() {
    return el('td', {}, [el('div', { className: 'ops-row-actions' }, [
      el('button', { type: 'button', disabled: true, title: readonly, text: '编辑' }),
      el('button', { type: 'button', disabled: true, title: readonly, text: '删除' }),
    ])]);
  }

  // ---------- 发布记录 ----------
  function viewsCell(views) {
    const cell = el('td', { className: 'ops-number' });
    const stack = el('div', { className: 'ops-stack' });
    stack.style.alignItems = 'flex-end';
    if (views.state === 'value') {
      const formatted = OpsFormat.count(views.value);
      stack.append(el('span', { text: formatted.text, title: formatted.title || undefined }));
      if (views.hit) stack.append(tag('爆款', 'success', '小红书阅读量不少于 5000'));
    } else if (views.state === 'pending') {
      stack.append(tag('待补', 'warning', '小红书阅读量还没人填，不是 0'));
      if (views.overdue) stack.append(tag('满 7 天', 'strong', '发布已满 7 天仍未填'));
    } else {
      stack.append(missing('抖音播放量选填，不判爆款'));
    }
    cell.append(stack);
    return cell;
  }
  function stateCell(item) {
    const views = item.views.state === 'value' ? '阅读量已填' : item.views.state === 'pending' ? '阅读量待补' : '抖音选填';
    return el('td', {}, [el('div', { className: 'ops-stack' }, [
      tag('未检查', null, '系统还没去主页核对过这篇帖子；自动发现上线后接入'),
      el('span', { className: 'ops-secondary', text: views }),
    ])]);
  }
  function titleCell(item) {
    if (item.title) return el('td', {}, [el('span', { className: 'ops-clamp', text: item.title, title: item.title })]);
    if (item.body) return el('td', {}, [el('span', { className: 'ops-clamp ops-secondary', text: item.body, title: '没有标题，显示正文开头' })]);
    return el('td', {}, [missing()]);
  }
  function linkCell(item) {
    const href = OpsFormat.safeHref(item.url);
    const stack = el('div', { className: 'ops-stack' }, [
      href ? el('a', { className: 'ops-ellipsis', href, target: '_blank', rel: 'noopener noreferrer', text: item.url, title: item.url })
        : el('span', { className: 'ops-ellipsis', text: item.url, title: item.url }),
    ]);
    if (item.duplicates.length) {
      const dates = item.duplicates.map((duplicate) => duplicate.businessDate).join('、');
      stack.append(tag(`另有 ${item.duplicates.length} 条重复登记`, null, `已合并的重复登记日期：${dates}`));
    }
    return el('td', {}, [stack]);
  }
  function mediaCell(item) {
    if (!item.cover.available) {
      return el('td', {}, [el('div', { className: 'ops-cover-missing', title: '这条记录在工作台没有封面，或封面还没拷来' }, [el('span', { className: 'ops-cover-glyph', 'aria-hidden': 'true', text: '▧' }), el('span', { text: '封面未迁入' })])]);
    }
    const src = `/ops/covers/${encodeURIComponent(item.ref)}`;
    const button = el('button', { type: 'button', className: 'ops-cover', 'aria-label': '查看封面大图' }, [
      el('img', { src, alt: '封面', loading: 'lazy' }),
    ]);
    button.addEventListener('click', () => {
      const dialog = document.getElementById('ops-cover-dialog');
      document.getElementById('ops-cover-large').src = src;
      dialog.showModal();
    });
    return el('td', {}, [button]);
  }
  function accountCell(item) {
    if (!item.account) return el('td', {}, [el('span', { className: 'ops-missing', text: '账号未知', title: '工作台里这条记录没有关联账号' })]);
    const stack = el('div', { className: 'ops-stack' }, [el('span', { className: 'ops-ellipsis', text: item.account.name, title: item.account.name })]);
    if (item.account.ledgerState !== 'in_ledger') stack.append(tag('已停用', null, '这个账号已在工作台删除，旧状态未知'));
    return el('td', {}, [stack]);
  }
  function publicationRow(item) {
    const date = el('div', { className: 'ops-stack' }, [el('span', { text: item.businessDate })]);
    if (item.dateState === 'suspect_year') date.append(tag('日期存疑', 'warning', '工作台里写成了 2024 年'));
    const rate = OpsFormat.percent(item.coverClickRate, 2);
    return el('tr', {}, [
      el('td', {}, [date]),
      el('td', { text: OpsFormat.platform(item.platform) }),
      titleCell(item),
      stateCell(item),
      linkCell(item),
      mediaCell(item),
      textCell(item.body),
      accountCell(item),
      textCell(item.publisher),
      disabledActions(),
      viewsCell(item.views),
      numberCell(item.likes),
      numberCell(item.collects),
      numberCell(item.comments),
      el('td', { className: 'ops-number' }, [rate.missing ? missing() : el('span', { text: rate.text })]),
      el('td', {}, [missing('选题池上线后指向对应选题')]),
    ]);
  }
  function hasPublicationFilters() {
    return ['operator', 'platform', 'account', 'from', 'to', 'views'].some((key) => params.get(key));
  }
  async function loadSummary() {
    const values = document.querySelectorAll('[data-readout]');
    try {
      const summary = await getJson(apiUrl('summary'));
      const put = (key, text, title) => {
        const node = document.querySelector(`[data-readout="${key}"]`);
        node.textContent = text;
        if (title) node.title = title;
      };
      const show = (value) => OpsFormat.count(value).text;
      put('accounts', summary.activeAccounts == null ? OpsFormat.missing : `${show(summary.activeAccounts)}/${show(summary.ledgerAccounts)}`);
      put('published', show(summary.monthPublished), `${summary.month}，北京时间`);
      put('followers', show(summary.monthFollowersGained.value), business === 'all' ? '' : '每日数据不分业务，这是全部业务的数字');
      put('hits', show(summary.monthHits), `${summary.month} 发布、阅读量不少于 5000 的小红书帖子`);
    } catch (error) {
      values.forEach((node) => { if (node.dataset.readout !== 'effective') node.textContent = OpsFormat.missing; });
      failed(loadSummary, '读数', error);
      return;
    }
    succeeded(loadSummary);
  }
  const selectPublisher = (name) => setFilter('operator', name);
  async function loadPublications() {
    const request = ++latestTableRequest;
    setBusy(true);
    try {
      const data = await getJson(apiUrl('publications', Object.fromEntries(params)));
      if (request !== latestTableRequest) return;
      renderTabs(data.operators, params.get('operator'), selectPublisher);
      if (!data.items.length && data.total > 0 && data.page > 1) {
        params.delete('page');
        syncUrl();
        loadPublications();
        return;
      }
      if (!data.items.length) {
        if (hasPublicationFilters()) {
          const clear = el('button', { type: 'button', text: '清除筛选' });
          clear.addEventListener('click', () => { ['operator', 'platform', 'account', 'from', 'to', 'views', 'page'].forEach((key) => params.delete(key)); syncUrl(); refreshControls(); loadPublications(); });
          emptyRow(16, '没有符合条件的发布记录。', clear);
        } else {
          emptyRow(16, data.asOf ? '没有发布记录。' : '还没有迁入数据。');
        }
      } else {
        rows.replaceChildren(...data.items.map(publicationRow));
      }
      document.getElementById('ops-page-summary').textContent =
        `第 ${data.page} / ${Math.max(data.totalPages, 1)} 页 · 共 ${data.total.toLocaleString('zh-CN')} 条 · 每页最多 ${data.pageSize} 条`;
      document.getElementById('ops-prev').disabled = data.page <= 1;
      document.getElementById('ops-next').disabled = data.page >= Math.max(data.totalPages, 1);
      succeeded(loadPublications);
    } catch (error) {
      if (request === latestTableRequest) failed(loadPublications, '发布记录', error);
    } finally {
      if (request === latestTableRequest) setBusy(false);
    }
  }
  function setFilter(key, value) {
    if (value) params.set(key, value); else params.delete(key);
    params.delete('page');
    syncUrl();
    refreshControls();
    loadPublications();
  }
  function refreshControls() {
    document.querySelectorAll('[data-platform]').forEach((button) => {
      button.setAttribute('aria-pressed', String((params.get('platform') || '') === button.dataset.platform));
    });
    const pending = document.getElementById('ops-pending');
    if (pending) pending.setAttribute('aria-pressed', String(params.get('views') === 'pending'));
    const account = document.getElementById('ops-account');
    if (account) account.value = params.get('account') || '';
    const dateButton = document.getElementById('ops-date-open');
    if (dateButton) {
      const from = params.get('from');
      const to = params.get('to');
      dateButton.textContent = from || to ? `${from || '开始'} ~ ${to || '结束'}` : '日期范围';
    }
  }
  function initPublications() {
    document.querySelectorAll('[data-platform]').forEach((button) => button.addEventListener('click', () => setFilter('platform', button.dataset.platform)));
    document.getElementById('ops-pending').addEventListener('click', () => setFilter('views', params.get('views') === 'pending' ? '' : 'pending'));
    document.getElementById('ops-account-search').addEventListener('submit', (event) => {
      event.preventDefault();
      setFilter('account', document.getElementById('ops-account').value.trim());
    });
    const dialog = document.getElementById('ops-date-dialog');
    const form = document.getElementById('ops-date-form');
    document.getElementById('ops-date-open').addEventListener('click', () => {
      form.elements.from.value = params.get('from') || '';
      form.elements.to.value = params.get('to') || '';
      dialog.showModal();
    });
    document.getElementById('ops-date-cancel').addEventListener('click', () => dialog.close());
    document.getElementById('ops-date-clear').addEventListener('click', () => {
      params.delete('from');
      params.delete('to');
      dialog.close();
      setFilter('from', '');
    });
    form.addEventListener('submit', () => {
      if (form.elements.from.value) params.set('from', form.elements.from.value); else params.delete('from');
      setFilter('to', form.elements.to.value);
    });
    document.getElementById('ops-prev').addEventListener('click', () => turnPage(-1));
    document.getElementById('ops-next').addEventListener('click', () => turnPage(1));
    refreshControls();
    loadSummary();
    loadPublications();
  }
  function turnPage(step) {
    const next = Math.max(1, Number(params.get('page') || 1) + step);
    params.set('page', String(next));
    syncUrl();
    loadPublications();
    wrap.scrollTop = 0;
  }

  // ---------- 账号管理 ----------
  function accountRow(item, index) {
    const name = OpsFormat.safeHref(item.profileUrl || '');
    const statusLabel = item.status ? OpsFormat.accountStatus[item.status] || item.status : null;
    return el('tr', {}, [
      el('td', { className: 'ops-number', text: String(index + 1) }),
      el('td', { text: OpsFormat.platform(item.platform) }),
      el('td', {}, [name ? el('a', { className: 'ops-ellipsis', href: name, target: '_blank', rel: 'noopener noreferrer', text: item.name, title: item.name })
        : el('span', { className: 'ops-ellipsis', text: item.name, title: item.name })]),
      textCell(item.deviceCode),
      numberCell(item.followerCount),
      textCell(item.ipLocation),
      textCell(item.operator),
      el('td', {}, [statusLabel ? el('span', { text: statusLabel }) : missing()]),
      el('td', {}, [tag('未填', null, '改为运营手动填写，录入功能上线后开放')]),
      el('td', {}, [item.isRetained === null || item.isRetained === undefined ? missing() : el('span', { text: item.isRetained ? '是' : '否' })]),
      textCell(item.notes),
      disabledActions(),
    ]);
  }
  function selectOperator(load) {
    return (name) => {
      if (name) params.set('operator', name); else params.delete('operator');
      syncUrl();
      load();
    };
  }
  async function loadAccounts() {
    const request = ++latestTableRequest;
    setBusy(true);
    try {
      const data = await getJson(apiUrl('accounts', { operator: params.get('operator') || '' }));
      if (request !== latestTableRequest) return;
      renderTabs(data.operators, params.get('operator'), selectOperator(loadAccounts));
      document.getElementById('ops-account-counts').textContent = `总账号 ${data.total} · 运营中 ${data.active}`;
      if (data.items.length) rows.replaceChildren(...data.items.map(accountRow));
      else emptyRow(12, data.asOf ? '当前筛选下没有账号。' : '还没有迁入数据。');
      succeeded(loadAccounts);
    } catch (error) {
      if (request === latestTableRequest) failed(loadAccounts, '账号', error);
    } finally {
      if (request === latestTableRequest) setBusy(false);
    }
  }

  // ---------- 每日数据 ----------
  function dailyRow(item) {
    const rate = OpsFormat.percent(item.addRate === null ? null : item.addRate * 100, 1);
    return el('tr', {}, [
      el('td', { text: item.businessDate }),
      el('td', { text: item.weekday }),
      textCell(item.operator),
      numberCell(item.published),
      numberCell(item.linksOpened),
      numberCell(item.followersGained),
      el('td', { className: 'ops-number' }, [rate.missing ? missing('打开为 0 时没有添加率') : el('span', { text: rate.text })]),
      numberCell(item.hits),
      numberCell(item.monthFollowersGained),
      numberCell(item.monthPublished),
      el('td', {}, [el('button', { type: 'button', disabled: true, title: readonly, text: '编辑' })]),
    ]);
  }
  async function loadDaily() {
    const request = ++latestTableRequest;
    setBusy(true);
    try {
      const data = await getJson(apiUrl('daily', { operator: params.get('operator') || '' }));
      if (request !== latestTableRequest) return;
      renderTabs(data.operators, params.get('operator'), selectOperator(loadDaily));
      if (data.items.length) rows.replaceChildren(...data.items.map(dailyRow));
      else emptyRow(11, data.asOf ? '当前筛选下没有每日数据。' : '还没有迁入数据。');
      succeeded(loadDaily);
    } catch (error) {
      if (request === latestTableRequest) failed(loadDaily, '每日数据', error);
    } finally {
      if (request === latestTableRequest) setBusy(false);
    }
  }

  const initialOperators = params.get('operator') ? [params.get('operator')] : [];
  if (page === 'publications') {
    renderTabs(initialOperators, params.get('operator'), selectPublisher);
    initPublications();
  }
  if (page === 'accounts') {
    renderTabs(initialOperators, params.get('operator'), selectOperator(loadAccounts));
    loadAccounts();
  }
  if (page === 'daily') {
    document.getElementById('ops-scope-note').hidden = business === 'all';
    renderTabs(initialOperators, params.get('operator'), selectOperator(loadDaily));
    loadDaily();
  }
})();
