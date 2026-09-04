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
      var modeInputs = Array.prototype.slice.call(ruleForm.querySelectorAll("input[name='mode']"));
      var automatic = ruleForm.querySelector("input[name='automatic_enabled']");
      var fixed = ruleForm.querySelector("select[name='fixed_interval_seconds']");
      var allDay = ruleForm.querySelector("input[name='all_day']");
      var windowInputs = Array.prototype.slice.call(ruleForm.querySelectorAll("[data-monitor-rule-window] input"));
      var dynamic = ruleForm.querySelector("[data-monitor-dynamic]");
      var readOnly = ruleForm.dataset.readonly === "true";

      function syncRuleControls() {
        var selected = modeInputs.find(function (input) { return input.checked; });
        var mode = selected ? selected.value : "";
        if (automatic) {
          automatic.disabled = readOnly || mode === "manual_only";
          if (automatic.disabled) {
            automatic.checked = false;
          }
        }
        if (fixed) {
          fixed.disabled = readOnly || mode !== "fixed";
        }
        if (dynamic) {
          dynamic.hidden = mode !== "dynamic";
        }
        windowInputs.forEach(function (input) {
          input.disabled = readOnly || Boolean(allDay && allDay.checked);
        });
      }

      modeInputs.forEach(function (input) {
        input.addEventListener("change", syncRuleControls);
      });
      if (allDay) {
        allDay.addEventListener("change", syncRuleControls);
      }
      syncRuleControls();
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
      setTaskText("[data-task-inspector-capabilities]", row.dataset.taskCapabilities);
      setTaskText("[data-task-inspector-created]", row.dataset.taskCreated);
      setTaskText("[data-task-inspector-failure]", row.dataset.taskFailure);
      setTaskText("[data-task-inspector-attempt]", row.dataset.taskAttempt);
      setTaskText("[data-task-inspector-package]", row.dataset.taskPackage);
      setTaskText("[data-task-inspector-receipt]", row.dataset.taskReceipt);
      setTaskText("[data-task-inspector-effect]", row.dataset.taskEffect);
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

  var drawer = document.getElementById("c-drawer");
  if (!drawer) {
    if (!ruleModal) {
      restoreTriggerFocus();
    }
    return;
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
