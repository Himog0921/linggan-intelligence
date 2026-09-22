/* COMMENT-STUDY-INTELLIGENCE-OVERVIEW-001
 * Read-only intelligence overview. Existing Targets / Pending / Problems / Runs remain the
 * authoritative drill-down surfaces; this file only composes facts already returned by them.
 */
(() => {
  'use strict';

  const overviewDomainRef = new URLSearchParams(window.location.search).get('domain');
  const overviewState = { kind: 'all', expression: null, method: 'solution', cache: null };
  const overviewNum = value => value == null || Number.isNaN(Number(value))
    ? '—'
    : new Intl.NumberFormat('zh-CN').format(Number(value));
  const overviewSum = values => Object.values(values || {}).reduce((sum, value) => sum + Number(value || 0), 0);
  const overviewPath = (path, extra = {}) => {
    const query = new URLSearchParams();
    if (overviewDomainRef) query.set('domain', overviewDomainRef);
    Object.entries(extra).forEach(([key, value]) => {
      if (value != null && value !== '') query.set(key, value);
    });
    return query.size ? `${path}?${query}` : path;
  };
  const overviewReadout = (text, value, unit, note, active = false) => `
    <div class="study-readout" data-active="${active}">
      <span class="study-readout-label">${esc(text)}</span>
      <div class="study-readout-value"><b>${esc(value)}</b>${unit ? `<span>${esc(unit)}</span>` : ''}</div>
      <span class="study-readout-note">${esc(note)}</span>
    </div>`;

  function overviewKinds(signals) {
    const counts = new Map();
    signals.forEach(signal => counts.set(signal.kind, (counts.get(signal.kind) || 0) + 1));
    return [...counts.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
  }
  function overviewExpressions(signals) {
    const groups = new Map();
    signals.forEach(signal => {
      if (signal.sourceState === 'restricted') return;
      const proposition = String(signal.proposition || '').trim();
      if (!proposition) return;
      const group = groups.get(proposition) || { proposition, signals: [] };
      group.signals.push(signal);
      groups.set(proposition, group);
    });
    return [...groups.values()]
      .sort((a, b) => b.signals.length - a.signals.length || a.proposition.localeCompare(b.proposition))
      .slice(0, 8);
  }
  function overviewProblemHighlights(problems, signals) {
    const current = new Map();
    signals.forEach(signal => {
      if (!signal.resolvedProblemRef) return;
      const group = current.get(signal.resolvedProblemRef) || [];
      group.push(signal);
      current.set(signal.resolvedProblemRef, group);
    });
    const active = (problems || []).filter(problem => problem.state === 'active');
    const changed = active
      .filter(problem => current.has(problem.problemRef))
      .map(problem => ({ problem, signals: current.get(problem.problemRef) }))
      .sort((a, b) => b.signals.length - a.signals.length || Number(b.problem.membershipCount) - Number(a.problem.membershipCount));
    if (changed.length) return changed.slice(0, 3);
    return active
      .sort((a, b) => Number(b.membershipCount) - Number(a.membershipCount))
      .slice(0, 3)
      .map(problem => ({ problem, signals: [] }));
  }
  function overviewUnresolved(latest) {
    if (!latest) return [];
    const rows = [];
    ['deferred_novel', 'deferred_ambiguous', 'deferred_context', 'retrieval_incomplete', 'budget_stopped', 'pending', 'protocol_rejected', 'failed']
      .forEach(state => {
        const count = Number(latest.resolutionStates?.[state] || 0);
        if (count) rows.push({ text: label(resolutionLabel, state) ?? state, count, view: 'pending' });
      });
    [['needs_context', '仍在等待评论语境'], ['failed', '评论研究处理失败'], ['excluded', '来源受限，未继续处理']]
      .forEach(([state, text]) => {
        const count = Number(latest.targetStates?.[state] || 0);
        if (count) rows.push({ text, count, view: 'targets' });
      });
    return rows;
  }
  function overviewVoices(data) {
    const byTarget = new Map();
    data.signals.forEach(signal => {
      const group = byTarget.get(signal.targetRef) || [];
      group.push(signal);
      byTarget.set(signal.targetRef, group);
    });
    let targets = data.targets.filter(target => target.commentText);
    if (overviewState.kind !== 'all') {
      targets = targets.filter(target => (byTarget.get(target.targetRef) || []).some(signal => signal.kind === overviewState.kind));
    }
    if (overviewState.expression) {
      targets = targets.filter(target => (byTarget.get(target.targetRef) || []).some(signal => signal.proposition === overviewState.expression));
    }
    targets.sort((a, b) => (byTarget.get(b.targetRef) || []).length - (byTarget.get(a.targetRef) || []).length || String(b.createdAt).localeCompare(String(a.createdAt)));
    return targets.slice(0, 4).map(target => ({ target, signals: byTarget.get(target.targetRef) || [] }));
  }

  function renderIntelligenceOverview(data) {
    const preview = data.setup?.sourcePreview || null;
    const latest = data.overview?.latestRun || null;
    const signalTotal = latest ? overviewSum(latest.signalStates) : null;
    const problemTotal = data.problems.length;
    const kinds = overviewKinds(data.signals);
    const maxKind = Math.max(1, ...kinds.map(([, count]) => count));
    const expressions = overviewExpressions(data.signals);
    const highlights = overviewProblemHighlights(data.problems, data.signals);
    const unresolved = overviewUnresolved(latest);
    const voices = overviewVoices(data);
    const titles = new Map((data.setup?.eligibleWorks || []).map(work => [work.workRef, work.title]));
    const methods = data.signals
      .filter(signal => signal.kind === overviewState.method && signal.sourceState !== 'restricted')
      .slice(0, 3);
    const scope = latest
      ? `最近一次研究 · ${String(latest.createdAt || '').slice(0, 16).replace('T', ' ')}`
      : '尚无研究运行';
    const signalCoverage = signalTotal == null
      ? '没有研究运行'
      : signalTotal > data.signals.length
        ? `当前读取 ${data.signals.length}/${signalTotal} 条信号`
        : `本次 ${signalTotal} 条信号`;
    const filterNote = overviewState.expression
      ? `当前仅看“${overviewState.expression}”关联的原声`
      : overviewState.kind !== 'all'
        ? `当前仅看${label(signalKindLabel, overviewState.kind) ?? overviewState.kind}类原声`
        : '按已关联信号数优先展示';

    return `<div class="study-overview">
      <div class="study-overview-scope"><span><strong>ADHD 评论研究</strong> · ${esc(scope)}</span><code>${esc(data.overview?.domainRef || overviewDomainRef || 'DOMAIN UNKNOWN')}</code></div>

      <section class="study-readout-strip" aria-label="评论研究观察基础">
        ${overviewReadout('观察到的评论', preview ? overviewNum(preview.totalCommentCount) : '—', '条', preview ? '当前领域库存快照' : '资格读取不可用')}
        ${overviewReadout('可研究评论', preview ? overviewNum(preview.eligibleCommentCount) : '—', '条', preview ? `覆盖 ${overviewNum(preview.works?.length || 0)} 篇作品` : '来源 gate 状态未知', true)}
        ${overviewReadout('本次接纳信号', signalTotal == null ? '—' : overviewNum(signalTotal), '条', latest ? '最近一次研究产出' : '尚无研究运行')}
        ${overviewReadout('长期用户问题', problemTotal === 100 ? '100+' : overviewNum(problemTotal), '个', problemTotal ? '累计已建档问题' : '尚无稳定问题')}
      </section>

      <div class="study-overview-grid">
        <section class="study-overview-section" aria-labelledby="study-kind-title">
          <div class="study-section-head"><h2 id="study-kind-title">用户表达构成</h2><span>${esc(signalCoverage)}</span></div>
          <p class="study-section-note">表达类型说明系统识别到了什么，不代表主题规模或需求强度。</p>
          ${kinds.length ? `<div class="study-kind-list">
            <button class="study-kind-row" type="button" data-overview-kind="all" aria-pressed="${overviewState.kind === 'all'}"><span class="study-kind-name">全部表达</span><span class="study-kind-track"><i style="width:100%"></i></span><span class="study-kind-count">${overviewNum(data.signals.length)}</span></button>
            ${kinds.map(([kind, count]) => `<button class="study-kind-row" type="button" data-overview-kind="${esc(kind)}" aria-pressed="${overviewState.kind === kind}"><span class="study-kind-name">${esc(label(signalKindLabel, kind) ?? kind)}</span><span class="study-kind-track"><i style="width:${Math.max(6, Math.round(count / maxKind * 100))}%"></i></span><span class="study-kind-count">${overviewNum(count)}</span></button>`).join('')}
          </div>` : '<p class="study-empty">最近一次研究还没有形成可读取的信号。</p>'}
        </section>

        <section class="study-overview-section" aria-labelledby="study-problem-title">
          <div class="study-section-head"><h2 id="study-problem-title">本期值得看的问题</h2><button class="study-link" type="button" data-study-view="problems">查看全部用户问题</button></div>
          <p class="study-section-note">优先显示本次新增关联；没有新增时显示累计依据较多的问题。不是需求排行榜。</p>
          ${highlights.length ? `<ol class="study-problem-list">${highlights.map((item, index) => {
            const evidence = item.signals.find(signal => signal.evidence)?.evidence;
            return `<li class="study-problem-item"><div class="study-problem-top"><span class="study-problem-index">0${index + 1}</span><h3 class="study-problem-title">${esc(item.problem.definition)}</h3></div><div class="study-problem-meta"><span>累计 ${overviewNum(item.problem.membershipCount)} 条关联信号</span>${item.signals.length ? `<span>本次 ${overviewNum(item.signals.length)} 条</span>` : '<span>长期积累</span>'}</div>${evidence ? `<blockquote class="study-problem-quote">“${esc(evidence)}”</blockquote>` : ''}</li>`;
          }).join('')}</ol>` : '<p class="study-empty">尚未形成稳定用户问题。已有原声和非问题类信号仍可继续查看。</p>'}
        </section>
      </div>

      <div class="study-overview-grid" data-balance="equal">
        <section class="study-overview-section" aria-labelledby="study-expression-title">
          <div class="study-section-head"><h2 id="study-expression-title">${expressions.some(group => group.signals.length > 1) ? '重复出现的归一表达' : '本次归一表达'}</h2><span>系统归一表达</span></div>
          <p class="study-section-note">这里不是词云；相同归一表达按不同 Signal 计数，点击后回到对应原声。</p>
          ${expressions.length ? `<div class="study-expression-list">${expressions.map(group => `<button class="study-expression" type="button" data-overview-expression="${esc(group.proposition)}" aria-pressed="${overviewState.expression === group.proposition}"><span>${esc(group.proposition)}</span><b>${overviewNum(group.signals.length)}</b></button>`).join('')}</div>` : '<p class="study-empty">当前没有可读取的归一表达。</p>'}
        </section>

        <section class="study-overview-section" aria-labelledby="study-method-title">
          <div class="study-section-head"><h2 id="study-method-title">用户提到的办法与经历</h2><span>不替用户下有效性结论</span></div>
          <div class="study-method-switcher" role="tablist" aria-label="办法与经历"><button type="button" role="tab" data-method-kind="solution" aria-selected="${overviewState.method === 'solution'}">解决办法</button><button type="button" role="tab" data-method-kind="experience" aria-selected="${overviewState.method === 'experience'}">使用经历</button></div>
          ${methods.length ? `<div class="study-method-list">${methods.map(signal => `<article class="study-method"><h3>${esc(signal.proposition || '未命名表达')}</h3>${signal.evidence ? `<blockquote>“${esc(signal.evidence)}”</blockquote>` : ''}<small>${esc(label(signalKindLabel, signal.kind) ?? signal.kind)}</small></article>`).join('')}</div>` : `<p class="study-empty">本次研究还没有可读取的${overviewState.method === 'solution' ? '解决办法' : '使用经历'}信号。</p>`}
        </section>
      </div>

      <div class="study-overview-grid">
        <section class="study-overview-section" aria-labelledby="study-voice-title">
          <div class="study-section-head"><h2 id="study-voice-title">代表原声</h2>${overviewState.kind !== 'all' || overviewState.expression ? '<button class="study-link" type="button" data-overview-clear>清除筛选</button>' : '<span>回到真实评论</span>'}</div>
          <p class="study-section-note">${esc(filterNote)}</p>
          ${voices.length ? `<div class="study-voice-list">${voices.map(({ target, signals }) => {
            const tags = [...new Set(signals.map(signal => label(signalKindLabel, signal.kind) ?? signal.kind))].slice(0, 3);
            return `<article class="study-voice"><div class="study-voice-meta"><div class="study-voice-tags">${tags.map(tag => `<span class="study-voice-tag">${esc(tag)}</span>`).join('')}</div><span>${overviewNum(target.signalCount)} 条信号</span></div><blockquote>“${esc(target.commentText)}”</blockquote><div class="study-voice-footer"><span>${esc(titles.get(target.workRef) || '来源作品')}</span><button class="study-link" type="button" data-study-view="targets">查看评论目标</button></div></article>`;
          }).join('')}</div>` : '<p class="study-empty">当前筛选下没有可读取的评论原声。</p>'}
        </section>

        <section class="study-overview-section" aria-labelledby="study-unknown-title">
          <div class="study-section-head"><h2 id="study-unknown-title">尚未看清的部分</h2><span>不是人工审核待办</span></div>
          <p class="study-section-note">只显示会影响当前理解的真实等待或失败状态。</p>
          ${unresolved.length ? `<div class="study-unknown-list">${unresolved.map(item => `<div class="study-unknown-row"><span>${esc(item.text)}</span><b>${overviewNum(item.count)}</b><button type="button" data-study-view="${esc(item.view)}">查看</button></div>`).join('')}</div>` : '<p class="study-empty">最近一次研究没有尚待说明的归并或处理状态。</p>'}
        </section>
      </div>

      <p class="study-overview-footnote">总览只读取已有研究事实。浏览、筛选与切换视图不会创建研究运行，也不会调用模型。</p>
    </div>`;
  }

  async function loadIntelligenceOverview() {
    const [overview, problems, runs, setup] = await Promise.all([
      get(overviewPath('overview')),
      get(overviewPath('problems', { limit: 100 })),
      get(overviewPath('runs', { limit: 50 })),
      get('setup').catch(() => null)
    ]);
    let targets = [];
    let signals = [];
    if (overview.latestRun?.runRef) {
      const [targetData, signalData] = await Promise.all([
        get(overviewPath('targets', { runRef: overview.latestRun.runRef, limit: 100 })),
        get(overviewPath('signals', { runRef: overview.latestRun.runRef, limit: 100 }))
      ]);
      targets = targetData.targets || [];
      signals = signalData.signals || [];
    }
    return { overview, problems: problems.problems || [], runs: runs.runs || [], setup, targets, signals };
  }
  async function renderIntelligenceOverviewTab() {
    overviewState.cache = await loadIntelligenceOverview();
    return renderIntelligenceOverview(overviewState.cache);
  }
  function rerenderIntelligenceOverview() {
    if (activeView !== 'overview' || !overviewState.cache) return;
    document.querySelector('#study-tab-result').innerHTML = renderIntelligenceOverview(overviewState.cache);
  }

  document.querySelector('#study-tab-result').addEventListener('click', event => {
    const view = event.target.closest('[data-study-view]');
    if (view) {
      switchToView(view.dataset.studyView);
      return;
    }
    const kind = event.target.closest('[data-overview-kind]');
    if (kind) {
      overviewState.kind = kind.dataset.overviewKind;
      overviewState.expression = null;
      rerenderIntelligenceOverview();
      return;
    }
    const expression = event.target.closest('[data-overview-expression]');
    if (expression) {
      const value = expression.dataset.overviewExpression;
      overviewState.expression = overviewState.expression === value ? null : value;
      overviewState.kind = 'all';
      rerenderIntelligenceOverview();
      return;
    }
    const method = event.target.closest('[data-method-kind]');
    if (method) {
      overviewState.method = method.dataset.methodKind;
      rerenderIntelligenceOverview();
      return;
    }
    if (event.target.closest('[data-overview-clear]')) {
      overviewState.kind = 'all';
      overviewState.expression = null;
      rerenderIntelligenceOverview();
    }
  });

  TAB_RENDERERS.overview = renderIntelligenceOverviewTab;
  const setupStatus = document.querySelector('#setup-status');
  const statusObserver = new MutationObserver(() => {
    setupStatus.hidden = !setupStatus.textContent.trim();
  });
  statusObserver.observe(setupStatus, { childList: true, characterData: true, subtree: true });
  setupStatus.hidden = !setupStatus.textContent.trim();

  if (activeView === 'overview') renderActiveTab();
})();
