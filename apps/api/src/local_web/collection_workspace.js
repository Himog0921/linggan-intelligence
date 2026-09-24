// Collection Workspace client behaviour.
//
// Navigation state belongs to the URL and is rendered on the server, so the first paint is
// already the right view — there is no client-side page switch and no flash of a wrong page.
// This file only owns behaviour that cannot be expressed by a link: focus containment for
// the rule modal, the drawer's tab selection and width toggle, and Escape dismissal.
(function () {
  "use strict";

  function restoreTriggerFocus() {
    var focusId = window.location.hash.slice(1);
    if (!focusId || (focusId.indexOf("target-") !== 0 && focusId.indexOf("monitor-rule-") !== 0)) {
      return;
    }
    var trigger = document.getElementById(focusId);
    if (trigger) {
      window.requestAnimationFrame(function () {
        trigger.focus();
      });
    }
  }

  function returnFromOverlay(overlay) {
    var returnUrl = overlay.dataset.returnUrl || "/collection/targets";
    // Keep the drawer's focus-return contract explicit for both the browser and the
    // server-rendered Collection tests. The rule modal remains a sibling overlay.
    if (typeof drawer !== "undefined" && overlay === drawer) {
      returnUrl = drawer.dataset.returnUrl || "/collection/targets";
    }
    var returnFocus = overlay.dataset.returnFocus;
    if (!returnFocus && typeof drawer !== "undefined" && overlay === drawer) {
      returnFocus = drawer.dataset.returnFocus;
    }
    if (returnFocus) {
      returnUrl += "#" + returnFocus;
    }
    window.location.assign(returnUrl);
  }

  function modalFocusable(modal) {
    return Array.prototype.slice.call(modal.querySelectorAll(
      "a[href],button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex='-1'])"
    )).filter(function (element) {
      return !element.hidden && element.getAttribute("aria-hidden") !== "true";
    });
  }

  var ruleModal = document.getElementById("c-monitor-rule");
  if (ruleModal) {
    var initialFocus = ruleModal.querySelector("[data-monitor-rule-initial-focus]");
    if (initialFocus) {
      window.requestAnimationFrame(function () {
        initialFocus.focus();
      });
    }

    ruleModal.addEventListener("keydown", function (event) {
      if (event.key === "Escape") {
        event.preventDefault();
        returnFromOverlay(ruleModal);
        return;
      }
      if (event.key !== "Tab") {
        return;
      }
      var focusable = modalFocusable(ruleModal);
      if (!focusable.length) {
        event.preventDefault();
        return;
      }
      var first = focusable[0];
      var last = focusable[focusable.length - 1];
      if (focusable.indexOf(document.activeElement) === -1) {
        event.preventDefault();
        (event.shiftKey ? last : first).focus();
      } else if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    });

    var ruleForm = ruleModal.querySelector("[data-monitor-rule-form]");
    if (ruleForm) {
      var automatic = ruleForm.querySelector("input[name='automatic_enabled']");
      var fixed = ruleForm.querySelector("select[name='fixed_interval_seconds']");
      var readOnly = ruleForm.dataset.readonly === "true";
      // The server accepts exactly one all-day fixed cadence.  Keep this
      // client script intentionally dull: it only applies the target's
      // read-only state and never reconstructs hidden calendar/mode logic.
      if (automatic) automatic.disabled = readOnly;
      if (fixed) fixed.disabled = readOnly;
    }
  }

  var attentionRows = Array.prototype.slice.call(document.querySelectorAll("[data-attention-row]"));
  var attentionInspector = document.querySelector("[data-attention-inspector]");
  if (attentionRows.length && attentionInspector) {
    function setAttentionText(selector, value) {
      var element = attentionInspector.querySelector(selector);
      if (element) {
        element.textContent = value || "UNKNOWN";
      }
    }
    function selectAttention(row) {
      attentionRows.forEach(function (candidate) {
        var selected = candidate === row;
        candidate.classList.toggle("is-selected", selected);
        candidate.setAttribute("aria-pressed", selected ? "true" : "false");
      });
      setAttentionText("[data-attention-reason]", row.dataset.controlReason);
      setAttentionText("[data-attention-title]", row.dataset.title);
      setAttentionText("[data-attention-observed]", row.dataset.observed);
      setAttentionText("[data-attention-detail]", row.dataset.detail);
      setAttentionText("[data-attention-owner]", row.dataset.owner);
      setAttentionText("[data-attention-action]", row.dataset.action);
    }
    attentionRows.forEach(function (row) {
      row.addEventListener("click", function () { selectAttention(row); });
    });
    selectAttention(attentionRows[0]);
  }

  var taskRows = Array.prototype.slice.call(document.querySelectorAll("[data-task-row]"));
  var taskInspector = document.querySelector("[data-task-inspector]");
  var taskFrozenTemplates = Array.prototype.slice.call(document.querySelectorAll("[data-task-frozen-template]"));
  if (taskRows.length && taskInspector) {
    function setTaskText(selector, value) {
      var element = taskInspector.querySelector(selector);
      if (element) {
        element.textContent = value || "UNKNOWN";
      }
    }
    function selectTask(row) {
      taskRows.forEach(function (candidate) {
        var selected = candidate === row;
        candidate.classList.toggle("is-selected", selected);
        candidate.setAttribute("aria-pressed", selected ? "true" : "false");
      });
      setTaskText("[data-task-inspector-ref]", "任务 #" + row.dataset.taskRef);
      setTaskText("[data-task-inspector-title]", row.dataset.taskTitle);
      setTaskText("[data-task-inspector-meta]", row.dataset.taskMeta);
      setTaskText("[data-task-inspector-state]", row.dataset.taskState);
      setTaskText("[data-task-inspector-state-note]", row.dataset.taskStateNote);
      var stateView = taskInspector.querySelector("[data-task-inspector-state-view]");
      var stateClasses = ["c-task-state-ok", "c-task-state-live", "c-task-state-warn", "c-task-state-wait"];
      if (stateView) {
        stateClasses.forEach(function (className) { stateView.classList.remove(className); });
        stateView.classList.add(stateClasses.indexOf(row.dataset.taskStateClass) >= 0 ? row.dataset.taskStateClass : "c-task-state-wait");
      }
      setTaskText("[data-task-inspector-capabilities]", row.dataset.taskCapabilities);
      setTaskText("[data-task-inspector-created]", row.dataset.taskCreated);
      setTaskText("[data-task-inspector-failure]", row.dataset.taskFailure);
      setTaskText("[data-task-inspector-attempt]", row.dataset.taskAttempt);
      setTaskText("[data-task-inspector-package]", row.dataset.taskPackage);
      setTaskText("[data-task-inspector-receipt]", row.dataset.taskReceipt);
      setTaskText("[data-task-inspector-effect]", row.dataset.taskEffect);
      var effectCode = taskInspector.querySelector("[data-task-inspector-effect-code]");
      if (effectCode) {
        // 机器码是旁注：没有码时留空，不能落成 UNKNOWN 那个假码。
        effectCode.textContent = row.dataset.taskEffectCode || "";
      }
      var frozen = taskInspector.querySelector("[data-task-inspector-frozen]");
      if (frozen) {
        var template = taskFrozenTemplates.find(function (candidate) {
          return candidate.dataset.taskFrozenTemplate === row.dataset.taskId;
        });
        if (template) {
          frozen.replaceChildren(template.content.cloneNode(true));
        } else {
          var empty = document.createElement("p");
          empty.className = "c-control-none";
          empty.textContent = "该任务没有关联的冻结 Work 投影。";
          frozen.replaceChildren(empty);
        }
      }
    }
    taskRows.forEach(function (row) {
      row.addEventListener("click", function () { selectTask(row); });
    });
    selectTask(taskRows[0]);
  }

  var taskTabs = Array.prototype.slice.call(document.querySelectorAll("[data-task-tab]"));
  var taskPanels = Array.prototype.slice.call(document.querySelectorAll("[data-task-panel]"));
  function selectTaskTab(tab, moveFocus) {
    taskTabs.forEach(function (candidate) {
      var selected = candidate === tab;
      candidate.classList.toggle("is-active", selected);
      candidate.setAttribute("aria-selected", selected ? "true" : "false");
      candidate.tabIndex = selected ? 0 : -1;
    });
    taskPanels.forEach(function (panel) {
      var selected = panel.dataset.taskPanel === tab.dataset.taskTab;
      panel.classList.toggle("is-active", selected);
      panel.hidden = !selected;
    });
    if (moveFocus) {
      tab.focus();
    }
  }
  taskTabs.forEach(function (tab) {
    tab.addEventListener("click", function () {
      selectTaskTab(tab, false);
    });
    tab.addEventListener("keydown", function (event) {
      var current = taskTabs.indexOf(tab);
      var next = null;
      if (event.key === "ArrowRight") {
        next = (current + 1) % taskTabs.length;
      } else if (event.key === "ArrowLeft") {
        next = (current - 1 + taskTabs.length) % taskTabs.length;
      } else if (event.key === "Home") {
        next = 0;
      } else if (event.key === "End") {
        next = taskTabs.length - 1;
      }
      if (next !== null) {
        event.preventDefault();
        selectTaskTab(taskTabs[next], true);
      }
    });
  });

  // The object name is the row's one real link. Mouse users may also click any
  // non-interactive data cell without turning the whole ARIA row into a nested link.
  var targetRows = Array.prototype.slice.call(document.querySelectorAll("[data-target-row]"));
  targetRows.forEach(function (row) {
    row.addEventListener("click", function (event) {
      var origin = event.target && event.target.closest ? event.target : null;
      if (origin && origin.closest("a,button,input,select,textarea,label,summary,[data-row-no-open]")) {
        return;
      }
      var opener = row.querySelector("[data-row-opener]");
      if (opener) {
        opener.click();
      }
    });
  });

  // Selection is deliberately separate from row navigation. A selected target is only a
  // candidate for the existing batch command; checking a box must never open its drawer.
  var targetSelections = Array.prototype.slice.call(document.querySelectorAll("[data-target-select]"));
  var targetSelectAll = Array.prototype.slice.call(document.querySelectorAll("[data-target-select-all]"));
  var targetBatchOpen = document.querySelector("[data-target-batch-open]");
  var targetBatchLabel = document.querySelector("[data-target-batch-label]");
  var targetSelectedCount = document.querySelector("[data-target-selected-count]");
  var targetBatchModal = document.querySelector("[data-target-batch-modal]");
  var targetBatchCount = document.querySelector("[data-target-batch-count]");
  var targetBatchClose = document.querySelector("[data-target-batch-close]");
  var targetBatchGroup = targetBatchModal && targetBatchModal.querySelector("input[name='group_name']");

  function selectedTargetCount() {
    return targetSelections.filter(function (input) { return input.checked; }).length;
  }

  function selectionsForHeader(input) {
    var section = input.closest(".c-tg-kind");
    return section ? Array.prototype.slice.call(section.querySelectorAll("[data-target-select]")) : targetSelections;
  }

  function syncTargetSelection() {
    var selected = selectedTargetCount();
    if (targetBatchOpen) targetBatchOpen.disabled = selected === 0;
    if (targetBatchLabel) targetBatchLabel.textContent = "批量编辑";
    if (targetSelectedCount) {
      targetSelectedCount.textContent = String(selected);
      targetSelectedCount.hidden = selected === 0;
    }
    if (targetBatchCount) targetBatchCount.textContent = "已选择 " + selected + " 个目标";
    targetSelectAll.forEach(function (input) {
      var scoped = selectionsForHeader(input);
      var scopedSelected = scoped.filter(function (candidate) { return candidate.checked; }).length;
      input.checked = scoped.length > 0 && scopedSelected === scoped.length;
      input.indeterminate = scopedSelected > 0 && scopedSelected < scoped.length;
    });
  }

  function closeTargetBatchModal() {
    if (!targetBatchModal) return;
    targetBatchModal.hidden = true;
    if (targetBatchOpen) targetBatchOpen.focus();
  }

  targetSelections.forEach(function (input) {
    input.addEventListener("change", syncTargetSelection);
  });
  targetSelectAll.forEach(function (input) {
    input.addEventListener("change", function () {
      selectionsForHeader(input).forEach(function (candidate) { candidate.checked = input.checked; });
      syncTargetSelection();
    });
  });
  if (targetBatchOpen && targetBatchModal) {
    targetBatchOpen.addEventListener("click", function () {
      if (selectedTargetCount() === 0) return;
      targetBatchModal.hidden = false;
      if (targetBatchGroup) targetBatchGroup.focus();
    });
    targetBatchModal.addEventListener("click", function (event) {
      if (event.target === targetBatchModal) closeTargetBatchModal();
    });
    targetBatchModal.addEventListener("keydown", function (event) {
      if (event.key === "Escape") {
        event.preventDefault();
        closeTargetBatchModal();
      }
    });
  }
  if (targetBatchClose) targetBatchClose.addEventListener("click", closeTargetBatchModal);
  syncTargetSelection();

  // 新建目标：先问领域，再提交。领域决定材料进本行业证据库还是跨行业参照语料，
  // 从前它由「当前在看哪个领域」推断，猜错会让参照物混进证据且不再有任何提示。
  var domainOpen = document.querySelector("[data-target-domain-open]");
  var domainModal = document.querySelector("[data-target-domain-modal]");
  var domainClose = document.querySelector("[data-target-domain-close]");
  var domainSelect = document.querySelector("[data-target-domain-select]");

  function closeDomainModal() {
    if (domainModal) domainModal.hidden = true;
    if (domainOpen) domainOpen.focus();
  }

  if (domainOpen && domainModal) {
    domainOpen.addEventListener("click", function () {
      var form = domainOpen.closest("form");
      // 目标本身的输入（类型、排序、链接/关键词）先过一遍浏览器校验，免得人选完领域
      // 才被告诉「关键词没填」。
      var identity = form && form.querySelector('[name="identity"]');
      if (identity && typeof identity.reportValidity === "function" && !identity.reportValidity()) return;
      domainModal.hidden = false;
      if (domainSelect) domainSelect.focus();
    });
    domainModal.addEventListener("click", function (event) {
      if (event.target === domainModal) closeDomainModal();
    });
    document.addEventListener("keydown", function (event) {
      if (event.key === "Escape" && !domainModal.hidden) closeDomainModal();
    });
  }
  if (domainClose) domainClose.addEventListener("click", closeDomainModal);

  var drawer = document.getElementById("c-drawer");
  if (!drawer) {
    if (!ruleModal) {
      restoreTriggerFocus();
    }
    return;
  }

  // A deep link with a fragment owns focus (for example a selected Work or an
  // archive problem). A first-open drawer without one starts at its labelled title.
  if (!window.location.hash) {
    var drawerInitialFocus = drawer.querySelector("[data-drawer-initial-focus]");
    if (drawerInitialFocus) {
      window.requestAnimationFrame(function () {
        drawerInitialFocus.focus();
      });
    }
  }

  var tabs = Array.prototype.slice.call(
    drawer.querySelectorAll(".c-drawer-tabs button[data-panel]")
  );
  var panels = Array.prototype.slice.call(
    drawer.querySelectorAll(".c-drawer-panel[data-panel]")
  );

  function selectPanel(name) {
    tabs.forEach(function (tab) {
      tab.classList.toggle("c-on", tab.dataset.panel === name);
    });
    panels.forEach(function (panel) {
      panel.hidden = panel.dataset.panel !== name;
    });
    syncUrl("tab", name === "overview" ? null : name);
  }

  // The drawer's own state stays shareable: reopening the same URL restores the same tab
  // and width. replaceState keeps it out of the back-button history, so Escape still
  // returns to the list rather than stepping through tab changes.
  function syncUrl(key, value) {
    if (!window.history || !window.history.replaceState) {
      return;
    }
    var url = new URL(window.location.href);
    if (value === null) {
      url.searchParams.delete(key);
    } else {
      url.searchParams.set(key, value);
    }
    window.history.replaceState(null, "", url);
  }

  tabs.forEach(function (tab) {
    tab.addEventListener("click", function () {
      selectPanel(tab.dataset.panel);
    });
  });

  var wideToggle = document.getElementById("c-drawer-wide");
  if (wideToggle) {
    wideToggle.addEventListener("click", function () {
      var wide = drawer.classList.toggle("c-wide");
      wideToggle.setAttribute("aria-pressed", wide ? "true" : "false");
      syncUrl("wide", wide ? "1" : null);
    });
  }

  document.addEventListener("keydown", function (event) {
    if (event.key === "Escape" && !ruleModal) {
      returnFromOverlay(drawer);
    }
  });

  var initial = new URL(window.location.href).searchParams;
  if (initial.get("wide") === "1") {
    drawer.classList.add("c-wide");
    if (wideToggle) {
      wideToggle.setAttribute("aria-pressed", "true");
    }
  }
  var requestedTab = initial.get("tab");
  if (requestedTab && tabs.some(function (tab) { return tab.dataset.panel === requestedTab; })) {
    selectPanel(requestedTab);
  }
})();

/* 单行工位表把低频真实动作收进原生弹窗：不恢复详情侧栏，也不让这些动作失去入口。 */
(function () {
  "use strict";

  var dialogs = Array.prototype.slice.call(document.querySelectorAll("[data-station-manage-dialog]"));
  if (!dialogs.length) return;

  dialogs.forEach(function (dialog) {
    var lastTrigger = null;
    var opener = document.querySelector("[data-station-manage-open][aria-controls='" + dialog.id + "']");
    var close = dialog.querySelector("[data-station-manage-close]");

    function restoreFocus() {
      if (lastTrigger && document.documentElement.contains(lastTrigger)) lastTrigger.focus();
      lastTrigger = null;
    }

    if (opener) {
      opener.addEventListener("click", function () {
        lastTrigger = opener;
        dialog.showModal();
        var first = dialog.querySelector("button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),a[href]");
        if (first) first.focus();
      });
    }
    if (close) close.addEventListener("click", function () { dialog.close(); });
    dialog.addEventListener("close", restoreFocus);
  });
})();

// 自绘下拉。
//
// 原生 select 的**弹出层由操作系统绘制**——自带圆角、蓝色高亮和阴影，CSS 一律管不到。
// 于是一个硬边、直角、墨线的页面，一点开筛选就露出四个系统菜单。证据库早就为此写了
// 一套自绘 select（见 evidence_library.js 的 createSelect），这里把同一件事做到观察
// 目标的工具条上。
//
// 用渐进增强而不是直接替换：原生 select 留在表单里当唯一真值来源，JS 只是在它上面画
// 一层。没有 JS 时表单照常提交——这是一个真的会 POST 的表单，不是查询面板。
(function () {
  "use strict";
  // 用标记属性而不是写死某一条工具条：抽屉里的「筛选作品」也是原生 select，
  // 同样需要这层自绘，否则一点开又是系统菜单。
  var fields = document.querySelectorAll("[data-drawn-select]");
  if (!fields.length) return;

  var openOne = null;
  function closeOpen() {
    if (!openOne) return;
    openOne.list.hidden = true;
    openOne.toggle.setAttribute("aria-expanded", "false");
    openOne = null;
  }
  document.addEventListener("click", closeOpen);
  document.addEventListener("keydown", function (event) {
    if (event.key === "Escape") closeOpen();
  });

  Array.prototype.forEach.call(fields, function (field) {
    var select = field.querySelector("select");
    if (!select) return;

    var toggle = document.createElement("button");
    toggle.type = "button";
    toggle.className = "c-tg-ctl c-tg-drawn-toggle";
    toggle.setAttribute("aria-haspopup", "listbox");
    toggle.setAttribute("aria-expanded", "false");
    var label = document.createElement("span");
    var caret = document.createElement("i");
    caret.className = "c-tg-caret";
    caret.setAttribute("aria-hidden", "true");
    toggle.appendChild(label);
    toggle.appendChild(caret);

    var list = document.createElement("div");
    list.className = "c-tg-drawn-list";
    list.setAttribute("role", "listbox");
    list.hidden = true;

    var buttons = [];
    Array.prototype.forEach.call(select.options, function (option) {
      var item = document.createElement("button");
      item.type = "button";
      item.setAttribute("role", "option");
      item.textContent = option.textContent;
      item.addEventListener("click", function (event) {
        event.stopPropagation();
        select.value = option.value;
        // 派发 change：隐藏关键词排序那段逻辑挂在原生 select 的 change 上，
        // 只改 value 不派发事件，它就不会跟着动。
        select.dispatchEvent(new Event("change", { bubbles: true }));
        paint();
        closeOpen();
        toggle.focus();
      });
      buttons.push({ button: item, value: option.value });
      list.appendChild(item);
    });

    function paint() {
      var current = select.options[select.selectedIndex];
      label.textContent = current ? current.textContent : "";
      buttons.forEach(function (entry) {
        entry.button.setAttribute("aria-selected", String(entry.value === select.value));
      });
    }

    toggle.addEventListener("click", function (event) {
      event.stopPropagation();
      var wasOpen = openOne && openOne.toggle === toggle;
      closeOpen();
      if (wasOpen) return;
      list.hidden = false;
      toggle.setAttribute("aria-expanded", "true");
      openOne = { toggle: toggle, list: list };
    });
    list.addEventListener("click", function (event) {
      event.stopPropagation();
    });
    select.addEventListener("change", paint);

    field.classList.add("c-tg-drawn");
    field.appendChild(toggle);
    field.appendChild(list);
    paint();
  });
})();

// 执行工位右侧的运行概览抽屉。
//
// 开合是纯视觉状态，不值得一次服务端往返，所以归客户端管：`data-open` 管位移，
// `inert` 管可达性。
//
// `inert` 是这里唯一不能省的一步。抽屉只是被挪出屏幕，节点还在文档里——不加 `inert`，
// 键盘 Tab 会一路走进一个看不见的面板，读屏也会念出屏幕外的那几条读数。所以关着的时候
// 它整块退出可达性，而不是只靠 transform 藏起来。
//
// 初始态是关。稿子把它画成开着的只是为了展示那四块内容；第一帧就压住正文、没有脚本又
// 永远关不掉的覆盖层，是这份稿子里最不该照抄的一处。
(function () {
  "use strict";

  var panel = document.getElementById("c-runtime-drawer");
  if (!panel) return;

  var openers = document.querySelectorAll(
    "[data-runtime-drawer-toggle],[data-runtime-drawer-open]"
  );
  var closeControl = panel.querySelector("[data-runtime-drawer-close]");
  var lastTrigger = null;

  function isOpen() {
    return panel.hasAttribute("data-open");
  }

  function paint(open) {
    if (open) {
      panel.setAttribute("data-open", "");
      panel.removeAttribute("inert");
    } else {
      panel.removeAttribute("data-open");
      panel.setAttribute("inert", "");
    }
    Array.prototype.forEach.call(openers, function (opener) {
      opener.setAttribute("aria-expanded", String(open));
    });
  }

  function openPanel(trigger) {
    lastTrigger = trigger || null;
    paint(true);
    if (closeControl) closeControl.focus();
  }

  function closePanel() {
    paint(false);
    // 焦点还回把它打开的那个控件。不还的话，焦点留在已经挪出屏幕的关闭按钮上，
    // 下一次 Tab 从屏幕外开始。
    if (lastTrigger && document.documentElement.contains(lastTrigger)) {
      lastTrigger.focus();
    }
    lastTrigger = null;
  }

  Array.prototype.forEach.call(openers, function (opener) {
    opener.addEventListener("click", function (event) {
      event.preventDefault();
      if (isOpen()) {
        closePanel();
      } else {
        openPanel(opener);
      }
    });
  });

  if (closeControl) closeControl.addEventListener("click", closePanel);

  document.addEventListener("keydown", function (event) {
    if (event.key !== "Escape" || !isOpen()) return;
    event.preventDefault();
    closePanel();
  });
})();

// 登记一台工位的居中弹窗。控制条上的「新增工位」开它。
//
// 关着的时候整块是 `hidden`（`display:none`），不是靠透明或位移藏起来——那样输入框仍在
// Tab 序列里，读屏也照样念得出。这是这里唯一不能省的一步。
//
// 初始态从服务端渲染的 `hidden` 读出来，不写死成「关」：一次失败的 POST 会把错误块渲染在
// 窗里并让它开着。若这里写死关闭，用户提交失败后看到的将是"页面什么都没说"，而原因就在
// 一个被脚本关掉的窗里。
(function () {
  "use strict";

  var overlay = document.querySelector("[data-station-register-overlay]");
  if (!overlay) return;

  var modal = overlay.querySelector(".c-stn-modal");
  var openers = document.querySelectorAll("[data-station-register-open]");
  var lastTrigger = null;

  function isOpen() {
    return !overlay.hasAttribute("hidden");
  }

  function paint(open) {
    if (open) {
      overlay.removeAttribute("hidden");
    } else {
      overlay.setAttribute("hidden", "");
    }
    Array.prototype.forEach.call(openers, function (opener) {
      opener.setAttribute("aria-expanded", String(open));
    });
  }

  function openModal(trigger) {
    lastTrigger = trigger || null;
    paint(true);
    // 先找输入框。写成 `"input, button"` 会选中排在它前面的「关闭」——窗头在表单之前。
    var field = modal && (modal.querySelector("input") || modal.querySelector("button"));
    if (field) field.focus();
  }

  function closeModal() {
    paint(false);
    // 焦点还回把它打开的那个按钮，否则关掉之后焦点落在被隐藏的输入框上，下一次 Tab
    // 从文档里一个看不见的位置开始。
    if (lastTrigger && document.documentElement.contains(lastTrigger)) {
      lastTrigger.focus();
    }
    lastTrigger = null;
  }

  Array.prototype.forEach.call(openers, function (opener) {
    opener.addEventListener("click", function () {
      if (isOpen()) {
        closeModal();
      } else {
        openModal(opener);
      }
    });
  });

  Array.prototype.forEach.call(
    overlay.querySelectorAll("[data-station-register-close]"),
    function (control) {
      control.addEventListener("click", closeModal);
    }
  );

  // 点遮罩关闭，点窗内不关。判据是目标节点就是遮罩本身——窗内任何位置都在 modal 里。
  overlay.addEventListener("click", function (event) {
    if (event.target === overlay) closeModal();
  });

  // 窗内可聚焦的元素，按文档顺序。选择器与页面上另一个弹窗（`#c-monitor-rule`）用的那一
  // 份逐字相同，别在这里另立一套。
  function focusable() {
    return Array.prototype.slice.call(modal.querySelectorAll(
      "a[href],button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex='-1'])"
    )).filter(function (element) {
      return !element.hidden && element.getAttribute("aria-hidden") !== "true";
    });
  }

  document.addEventListener("keydown", function (event) {
    if (!isOpen()) return;
    if (event.key === "Escape") {
      event.preventDefault();
      closeModal();
      return;
    }
    if (event.key !== "Tab") return;
    // 这个窗声明了 `aria-modal="true"`，读屏会据此不再念后面的页面。若 Tab 还能走到窗外，
    // 声明与实际行为就对不上：用户会落到一个读屏不说、视觉上也被遮罩压暗的位置。所以按
    // 页面既有弹窗的做法把 Tab 圈在窗内，而不是给整页加 `inert`（那要反过来排除本窗，
    // 牵动的是整页）。
    var items = focusable();
    if (!items.length) {
      event.preventDefault();
      return;
    }
    var first = items[0];
    var last = items[items.length - 1];
    if (items.indexOf(document.activeElement) === -1) {
      event.preventDefault();
      (event.shiftKey ? last : first).focus();
    } else if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  });

  // 首帧同步一次：服务端可能是带着 `hidden` 渲染的（常态），也可能是带着错误块开着渲染
  // 的（提交失败）。上面那条 `paint` 之外的唯一作用是让 `aria-expanded` 与真实状态一致。
  paint(isOpen());
})();

/* runtime-station-board-v9-final
 * 工位不再打开详情侧栏。名称只做行内编辑；接活按钮继续由服务端 POST 处理。
 */
(function () {
  "use strict";

  var rows = Array.prototype.slice.call(document.querySelectorAll(".c-stn-row"));
  if (!rows.length) return;

  function closeEditor(row) {
    var form = row.querySelector("[data-station-name-form]");
    var label = row.querySelector(".c-stn-name-label");
    var edit = row.querySelector("[data-station-name-edit]");
    if (!form || form.hidden) return;
    form.hidden = true;
    if (label) label.hidden = false;
    if (edit) edit.hidden = false;
  }

  rows.forEach(function (row) {
    var edit = row.querySelector("[data-station-name-edit]");
    var form = row.querySelector("[data-station-name-form]");
    var label = row.querySelector(".c-stn-name-label");
    var cancel = row.querySelector("[data-station-name-cancel]");
    var input = row.querySelector(".c-stn-name-input");

    if (edit && form && label && input) {
      edit.addEventListener("click", function (event) {
        event.preventDefault();
        rows.forEach(function (other) {
          if (other !== row) closeEditor(other);
        });
        label.hidden = true;
        edit.hidden = true;
        form.hidden = false;
        window.requestAnimationFrame(function () {
          input.focus();
          input.select();
        });
      });
    }

    if (cancel) {
      cancel.addEventListener("click", function (event) {
        event.preventDefault();
        closeEditor(row);
        if (edit) edit.focus();
      });
    }
  });

  document.addEventListener("keydown", function (event) {
    if (event.key !== "Escape") return;
    var active = rows.find(function (row) {
      var form = row.querySelector("[data-station-name-form]");
      return form && !form.hidden;
    });
    if (!active) return;
    event.preventDefault();
    var edit = active.querySelector("[data-station-name-edit]");
    closeEditor(active);
    if (edit) edit.focus();
  });
})();
