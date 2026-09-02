(() => {
  "use strict";
  const root = document.querySelector("[data-topic-workspace]");
  if (!root) return;

  const state = { materials: [], filter: "all", selected: null };
  const byId = (id) => document.getElementById(id);
  const setText = (id, value) => { const node = byId(id); if (node) node.textContent = value; };
  const roleLabel = { support: "支持", challenge: "挑战", boundary: "边界" };

  const fact = (label, value) => {
    const row = document.createElement("div");
    const term = document.createElement("dt");
    const detail = document.createElement("dd");
    term.textContent = label;
    detail.textContent = value ?? "未知";
    row.append(term, detail);
    return row;
  };

  function renderInspector(material) {
    state.selected = material?.classification?.workPublicRef ?? null;
    document.querySelectorAll(".topic-material").forEach((node) => {
      node.setAttribute("aria-selected", String(node.dataset.ref === state.selected));
    });
    const facts = byId("topic-inspector-facts");
    facts.replaceChildren();
    const link = byId("topic-inspector-link");
    if (!material) {
      setText("topic-inspector-title", "请选择一条材料");
      setText("topic-inspector-rationale", "右侧只显示当前 Work Resource 读模型与人工裁定理由。");
      link.hidden = true;
      return;
    }
    const work = material.workResource;
    const display = work.display ?? {};
    setText("topic-inspector-title", display.title ?? "标题未知");
    setText("topic-inspector-rationale", material.classification.rationale);
    facts.append(
      fact("裁定角色", roleLabel[material.classification.role] ?? material.classification.role),
      fact("Work Resource", material.classification.workPublicRef),
      fact("平台身份", `${work.identity?.platform ?? "未知"} / ${work.identity?.contentExternalId ?? "未知"}`),
      fact("创作者", display.creatorDisplayName ?? "未知"),
      fact("来源发布时间", display.publishedAt ?? display.publishedAtSourceText ?? "未知"),
      fact("主要限制", work.summary?.primaryLimitation ?? "未知")
    );
    link.href = material.detailUrl;
    link.hidden = false;
  }

  function materialButton(material, index) {
    const classification = material.classification;
    const work = material.workResource;
    const button = document.createElement("button");
    button.type = "button";
    button.className = "topic-material";
    button.dataset.role = classification.role;
    button.dataset.ref = classification.workPublicRef;
    button.setAttribute("role", "option");
    button.setAttribute("aria-selected", "false");
    const ordinal = document.createElement("span");
    ordinal.className = "topic-material-index";
    ordinal.textContent = String(index + 1).padStart(2, "0");
    const main = document.createElement("span");
    main.className = "topic-material-main";
    const title = document.createElement("strong");
    title.className = "topic-material-title";
    title.textContent = work.display?.title ?? "标题未知";
    const meta = document.createElement("span");
    meta.className = "topic-material-meta";
    meta.textContent = `${work.identity?.platform ?? "未知"} · ${work.identity?.contentExternalId ?? "未知"} · ${work.display?.creatorDisplayName ?? "创作者未知"}`;
    main.append(title, meta);
    const role = document.createElement("span");
    role.className = "topic-role";
    role.dataset.role = classification.role;
    role.textContent = `${roleLabel[classification.role] ?? classification.role}材料`;
    button.append(ordinal, main, role);
    button.addEventListener("click", () => renderInspector(material));
    return button;
  }

  function renderMaterials() {
    const list = byId("topic-material-list");
    list.replaceChildren();
    const visible = state.materials.filter((material) => state.filter === "all" || material.classification.role === state.filter);
    visible.forEach((material) => list.append(materialButton(material, state.materials.indexOf(material))));
    setText("topic-material-count", String(visible.length));
    if (visible.length === 0) {
      const empty = document.createElement("p");
      empty.className = "topic-feedback";
      empty.textContent = "当前裁定视角没有材料；这不代表来源世界中不存在相关内容。";
      list.append(empty);
      renderInspector(null);
    } else {
      renderInspector(visible.find((item) => item.classification.workPublicRef === state.selected) ?? visible[0]);
    }
  }

  function renderWorkspace(payload) {
    const topic = payload.topic;
    state.materials = payload.materials ?? [];
    setText("topic-title", topic.definition.displayName);
    setText("topic-definition-text", topic.definition.definitionText);
    setText("topic-version", `VERSION ${topic.definition.version}`);
    setText("topic-run-kind", "人工裁定 / HUMAN ADJUDICATED");
    setText("topic-pack-ref", topic.materialPack.materialPackRef);
    setText("topic-read-state", "已读取当前暂定版本");
    setText("topic-adjudication-note", topic.classificationRun.adjudicationNote);
    setText("topic-source-boundary", topic.materialPack.sourceBoundary);
    setText("topic-count-all", String(state.materials.length));
    for (const role of ["support", "challenge", "boundary"]) {
      setText(`topic-count-${role}`, String(state.materials.filter((item) => item.classification.role === role).length));
    }
    const feedback = byId("topic-feedback");
    feedback.dataset.state = "ready";
    feedback.textContent = `已读取定义 v${topic.definition.version} 与冻结材料包；材料内容来自共享 Work Resource Read。`;
    renderMaterials();
  }

  function renderUnavailable(code) {
    const feedback = byId("topic-feedback");
    feedback.dataset.state = "unavailable";
    feedback.textContent = `当前未读取 Topic（${code}）。未显示缓存、合成材料或旧系统结果。`;
    setText("topic-read-state", "当前未读取 Topic");
    setText("topic-material-count", "—");
  }

  document.querySelectorAll("[data-role-filter]").forEach((button) => {
    button.addEventListener("click", () => {
      state.filter = button.dataset.roleFilter;
      document.querySelectorAll("[data-role-filter]").forEach((item) => item.setAttribute("aria-pressed", String(item === button)));
      renderMaterials();
    });
  });

  const key = root.dataset.topicKey;
  fetch(`/api/local/topic-workspaces/${encodeURIComponent(key)}`, { headers: { Accept: "application/json" } })
    .then(async (response) => {
      const payload = await response.json().catch(() => ({}));
      if (!response.ok) throw new Error(payload.code ?? `HTTP_${response.status}`);
      return payload;
    })
    .then(renderWorkspace)
    .catch((error) => renderUnavailable(error.message));
})();
