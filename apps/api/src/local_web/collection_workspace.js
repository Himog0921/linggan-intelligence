// Collection Workspace client behaviour.
//
// Navigation state belongs to the URL and is rendered on the server, so the first paint is
// already the right view — there is no client-side page switch and no flash of a wrong page.
// This file only owns behaviour that cannot be expressed by a link: the drawer's tab
// selection, its width toggle, and closing it with Escape.
(function () {
  "use strict";

  function restoreDrawerTriggerFocus() {
    var focusId = window.location.hash.slice(1);
    if (!focusId || focusId.indexOf("target-") !== 0) {
      return;
    }
    var trigger = document.getElementById(focusId);
    if (trigger) {
      window.requestAnimationFrame(function () {
        trigger.focus();
      });
    }
  }

  var drawer = document.getElementById("c-drawer");
  if (!drawer) {
    restoreDrawerTriggerFocus();
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
    if (event.key === "Escape") {
      var returnUrl = drawer.dataset.returnUrl || "/collection/targets";
      var returnFocus = drawer.dataset.returnFocus;
      if (returnFocus) {
        returnUrl += "#" + returnFocus;
      }
      window.location.assign(returnUrl);
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
