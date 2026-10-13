/* Shared Domain manager: list first; existing concrete forms and command routes in one dialog. */
(() => {
  'use strict';
  const entry = document.querySelector('a[data-collection-management][href^="/collection/domains"]');
  if (!entry) return;
  const dialog = document.createElement('dialog');
  dialog.className = 'c-run-dialog c-domain-manager';
  dialog.setAttribute('aria-labelledby', 'domain-manager-title');
  dialog.innerHTML = '<header><div><h2 id="domain-manager-title">领域管理</h2><p>整理关注范围，在同一个工作台切换采集上下文。</p></div><button class="c-btn-quiet" type="button" aria-label="关闭领域管理" data-close-domain>×</button></header><div class="c-run-dialog-body" data-domain-content></div><footer><button type="button" class="c-btn-secondary" data-close-domain>完成</button></footer>';
  (document.querySelector('.c-page') || document.body).append(dialog);
  const body = dialog.querySelector('[data-domain-content]');
  const originalUrl = location.href;
  const escape = value => String(value ?? '').replace(/[&<>"']/g, c => ({ '&':'&amp;', '<':'&lt;', '>':'&gt;', '"':'&quot;', "'":'&#39;' }[c]));
  let editing = null;
  let returnFocus = entry;
  dialog.querySelectorAll('[data-close-domain]').forEach(b => b.addEventListener('click', () => dialog.close()));
  dialog.addEventListener('close', () => { document.documentElement.classList.remove('c-run-dialog-open'); returnFocus?.focus(); });
  async function list() {
    editing = null; body.textContent = '正在读取领域…';
    try {
      const response = await fetch('/api/collection/management/domains');
      if (!response.ok) throw new Error('领域读取失败');
      const data = await response.json();
      const current=new URL(originalUrl).searchParams.get('domain');
      body.innerHTML = '<div class="c-run-toolbar"><b>' + data.items.length + ' 个领域</b><button class="c-btn-secondary" type="button" data-edit-domain="new">＋ 新建领域</button></div>' + data.items.map(d => '<article class="c-domain-list-row"><div><b>' + escape(d.name) + '</b>'+(d.domainRef===current?' <small class="c-domain-current">当前</small>':'')+'<p>' + escape(d.description || '暂无领域说明') + '</p></div><span>' + escape(d.targetCount) + ' 目标</span><span class="c-domain-status" data-active="'+(d.status==='active')+'">' + (d.status === 'active' ? '运行中' : '已暂停') + '</span><div class="c-run-controls"><button type="button" class="c-btn-quiet" data-enter-domain="' + escape(d.domainRef) + '">进入</button><button type="button" class="c-btn-quiet" data-edit-domain="' + escape(d.domainRef) + '">编辑</button><button type="button" class="c-btn-quiet" data-toggle-domain="' + escape(d.domainRef) + '">' + (d.status === 'active' ? '暂停' : '恢复') + '</button></div></article>').join('')+'<p class="c-domain-note">暂停领域会暂缓该领域的新采集和排队任务，历史材料仍可查看；已取得的数据继续交付。目标可被多个领域关联，详情按来源复用。</p>';
      body.scrollTop = 0;
    } catch (error) { body.textContent = error.message + '；这不表示没有领域。'; }
  }
  async function edit(id) {
    const response = await fetch('/api/collection/management/domains/' + encodeURIComponent(id));
    if (!response.ok) throw new Error('领域编辑内容读取失败');
    const html = await response.text();
    const doc = new DOMParser().parseFromString(html, 'text/html');
    const component = doc.querySelector('.domain-card, .domain-create');
    if (!component) throw new Error('领域编辑组件当前不可用');
    editing = id; body.replaceChildren();
    const back = document.createElement('button'); back.type = 'button'; back.className = 'c-btn-quiet'; back.textContent = '← 返回领域列表'; back.dataset.domainBack = '';
    body.append(back, document.importNode(component, true));
    const statistics = body.querySelector('.domain-counts');
    if (statistics) { const details = document.createElement('details'); const summary = document.createElement('summary'); summary.textContent = '材料统计与通道'; details.append(summary); statistics.before(details); details.append(statistics); const lanes = body.querySelector('.domain-lanes'); if (lanes) details.append(lanes); }
    body.scrollTop = 0;
    body.querySelector('input[name=name]')?.focus({preventScroll: true});
  }
  entry.addEventListener('click', event => {
    event.preventDefault(); returnFocus = document.activeElement;
    entry.closest('details')?.removeAttribute('open');
    dialog.showModal(); document.documentElement.classList.add('c-run-dialog-open'); list();
  });
  body.addEventListener('click', async event => {
    const b = event.target.closest('button'); if (!b) return;
    if (b.hasAttribute('data-domain-back')) { list(); return; }
    if (b.dataset.enterDomain) { const destination = new URL(originalUrl); destination.searchParams.set('domain', b.dataset.enterDomain); destination.searchParams.delete('page'); location.assign(destination.href); return; }
    try {
      if (b.dataset.editDomain) await edit(b.dataset.editDomain);
      if (b.dataset.toggleDomain) {
        await edit(b.dataset.toggleDomain);
        const form = body.querySelector('.domain-fields');
        form.elements.status.value = form.elements.status.value === 'active' ? 'paused' : 'active';
        form.requestSubmit();
      }
    } catch (error) { const notice = document.createElement('p'); notice.setAttribute('role', 'alert'); notice.textContent = error.message; body.append(notice); }
  });
  body.addEventListener('submit', async event => {
    const form = event.target; if (!(form instanceof HTMLFormElement)) return;
    // The retained relation forms have a field named "action", which shadows form.action.
    const action = new URL(form.getAttribute('action'), location.href);
    if (action.origin !== location.origin || !action.pathname.startsWith('/collection/domains/')) return;
    event.preventDefault();
    const buttons = [...form.querySelectorAll('button')]; buttons.forEach(b => b.disabled = true);
    const notice = document.createElement('p'); notice.setAttribute('role', 'alert'); form.append(notice);
    try {
      const response = await fetch(action.pathname, { method: 'POST', body: new URLSearchParams(new FormData(form)) });
      const url = new URL(response.url);
      const html = await response.text();
      const doc = new DOMParser().parseFromString(html, 'text/html');
      if (!response.ok || url.searchParams.has('error') || doc.querySelector('.domain-feedback-error, .domain-draft')) throw new Error(doc.querySelector('.domain-feedback-error')?.textContent || '保存未确认；草稿已保留。');
      if (form.classList.contains('domain-fields') || form.classList.contains('domain-create')) await list();
      else await edit(editing);
    } catch (error) { notice.textContent = error.message; }
    finally { buttons.forEach(b => b.disabled = false); }
  });
})();
