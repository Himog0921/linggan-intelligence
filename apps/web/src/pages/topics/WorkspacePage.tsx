import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useParams } from "react-router";
import { AppShell } from "../../app/AppShell";
import { ApiError } from "../../shared/api";
import styles from "./Workspace.module.css";
import {
  readWorkspace,
  type WorkspaceMaterial,
  type WorkspaceRole,
} from "./workspaceApi";
const c = (...names: string[]) =>
  names.map((name) => styles[name] || name).join(" ");
const roles = { support: "支持", challenge: "挑战", boundary: "边界" };
const filters = ["all", "support", "challenge", "boundary"] as const;
type RoleFilter = (typeof filters)[number];
const label = (role: WorkspaceRole) => roles[role];
const initialRationale = "右侧显示当前材料读模型与分类依据。";
function errorCode(error: Error) {
  return error instanceof ApiError && error.failure.kind === "http"
    ? error.failure.code || `HTTP_${error.failure.status}`
    : "返回内容无法解析";
}
function Inspector({ material, machine }: { material: WorkspaceMaterial | undefined; machine: boolean }) {
  const facts = material
    ? [
        [machine ? "分类角色" : "裁定角色", label(material.role)],
        ["Work Resource", material.ref],
        ["平台身份", `${material.platform} / ${material.externalId}`],
        ["创作者", material.creator ?? "未知"],
        [
          "来源发布时间",
          material.publishedAt ?? material.publishedAtSourceText ?? "未知",
        ],
        ["主要限制", material.primaryLimitation],
      ]
    : [];
  return (
    <aside
      className={c("topic-inspector")}
      aria-labelledby="topic-inspector-title"
    >
      <p className={c("topic-panel-label")}>
        当前材料 <small>CURRENT MATERIAL</small>
      </p>
      <h2 id="topic-inspector-title">
        {material ? (material.title ?? "标题未知") : "请选择一条材料"}
      </h2>
      <p id="topic-inspector-rationale">
        {material?.rationale ?? initialRationale}
      </p>
      <dl id="topic-inspector-facts">
        {facts.map(([term, detail]) => (
          <div key={term}>
            <dt>{term}</dt>
            <dd>{detail}</dd>
          </div>
        ))}
      </dl>
      <a
        id="topic-inspector-link"
        hidden={!material}
        href={material?.detailUrl}
      >
        打开 Work Resource JSON
      </a>
    </aside>
  );
}
function useWorkspace(canonicalKey: string) {
  return useQuery({
    queryKey: ["topics", "workspace", canonicalKey],
    queryFn: ({ signal }) => readWorkspace(canonicalKey, signal),
    retry: false,
    staleTime: 0,
    gcTime: 0,
    refetchOnWindowFocus: false,
  });
}
function WorkspaceContents({ query, canonicalKey }: { query: ReturnType<typeof useWorkspace>; canonicalKey: string }) {
  const [filter, setFilter] = useState<RoleFilter>("all");
  const [selected, setSelected] = useState<string | null>(null);
  const data = query.isSuccess && !query.isFetching ? query.data : undefined;
  const visible =
    data?.materials.filter((m) => filter === "all" || m.role === filter) ?? [];
  const material = visible.find((m) => m.ref === selected) ?? visible[0];
  const topic = data?.topic;
  const machine = topic?.runKind === "machine_proposed";
  const lifecycle = !topic
    ? { label: "主题未读取", key: "TOPIC NOT READ", read: "当前未读取 Topic" }
    : topic.definition.lifecycleState === "candidate"
      ? { label: "候选主题", key: "CANDIDATE TOPIC", read: "已读取当前候选版本" }
      : { label: "暂定主题", key: "PROVISIONAL TOPIC", read: "已读取当前暂定版本" };
  function chooseFilter(next: RoleFilter) {
    setFilter(next);
    const nextVisible =
      data?.materials.filter((item) => next === "all" || item.role === next) ??
      [];
    // The old inspector selected the first visible member when its current member was hidden.
    setSelected(
      nextVisible.find((item) => item.ref === material?.ref)?.ref ??
        nextVisible[0]?.ref ??
        null,
    );
  }
  const feedback = query.isError
    ? `当前未读取 Topic（${errorCode(query.error)}）。未显示缓存、合成材料或旧系统结果。`
    : data
      ? `已读取定义 v${data.topic.definition.version} 与冻结材料包；材料内容来自共享 Work Resource Read。`
      : "正在读取本机 Topic 工作区…";
  return (
    <div
      className="v7-shell"
      data-topic-workspace
      data-topic-key={canonicalKey}
    >
      <aside className={c("v7-side", "topic-rail")} aria-label="Topic 研究导航">
        <div className="v7-nav-label" data-readout="TOPIC">
          主题研究
        </div>
        <a className="v7-side-nav" href="#definition" aria-current="page">
          <i>01</i>
          <span>定义与版本</span>
        </a>
        <a className="v7-side-nav" href="#materials">
          <i>02</i>
          <span>{machine || !topic ? "材料分类" : "材料裁定"}</span>
        </a>
        <a className="v7-side-nav" href="#boundary">
          <i>03</i>
          <span>来源边界</span>
        </a>
        <div className="v7-side-foot">
          <span className="v7-side-dot" />
          {lifecycle.label}
          <br />
          <span className="v7-tech-key">{lifecycle.key}</span>
          <br />
          不构成正式 Topic、趋势或市场事实。
        </div>
      </aside>
      <main
        data-topic-workspace-main=""
        className={c("topic-workspace")}
        aria-labelledby="topic-title"
      >
        <section className={c("topic-definition")} id="definition">
          <div>
            <p className={c("topic-eyebrow")}>领域探索 / TOPIC WORKSPACE</p>
            <div className={c("topic-title-row")}>
              <span className={c("topic-state")}>
                {lifecycle.label} <small>{lifecycle.key}</small>
              </span>
              <span id="topic-version">
                {topic
                  ? `VERSION ${topic.definition.version}`
                  : "VERSION UNKNOWN"}
              </span>
            </div>
            <h1 id="topic-title">
              {topic?.definition.displayName ?? "当前未读取 Topic"}
            </h1>
            <p id="topic-definition-text">
              {topic?.definition.definitionText ??
                "页面只声明研究结构；本机读模型返回前，不显示合成定义或材料。"}
            </p>
          </div>
          <dl className={c("topic-facts")}>
            <div>
              <dt>裁定方式</dt>
              <dd id="topic-run-kind">
                {topic ? (
                  topic.runKind === "machine_proposed" ? "机器候选 / MACHINE PROPOSED" : "人工裁定 / HUMAN ADJUDICATED"
                ) : (
                  <>
                    裁定方式未读取
                  </>
                )}
              </dd>
            </div>
            <div>
              <dt>材料包</dt>
              <dd id="topic-pack-ref">
                {topic?.materialPackRef ?? "尚未读取"}
              </dd>
            </div>
            <div>
              <dt>读取状态</dt>
              <dd id="topic-read-state">
                {lifecycle.read}
              </dd>
            </div>
          </dl>
        </section>
        <section
          className={c("topic-research")}
          id="materials"
          aria-label="Topic 材料研究面"
        >
          <aside className={c("topic-roles")}>
            <p className={c("topic-panel-label")}>
              分类视角 <small>CLASSIFICATION</small>
            </p>
            {filters.map((role) => (
              <button
                type="button"
                key={role}
                data-role-filter={role}
                aria-pressed={filter === role}
                onClick={() => chooseFilter(role)}
              >
                {[role === "all" ? (machine ? "全部分类" : "全部裁定") : `${label(role)}材料`, " "].join(
                  "",
                )}
                <b id={`topic-count-${role}`}>
                  {data
                    ? role === "all"
                      ? data.materials.length
                      : data.materials.filter((m) => m.role === role).length
                    : "—"}
                </b>
              </button>
            ))}
            <div className={c("topic-adjudication")}>
              <strong>{topic ? machine ? "机器分类说明" : "人工裁定说明" : "分类说明"}</strong>
              <p id="topic-adjudication-note">
                {topic?.adjudicationNote ?? "尚未读取。"}
              </p>
            </div>
          </aside>
          <section
            className={c("topic-materials")}
            aria-labelledby="topic-materials-title"
          >
            <header>
              <div>
                <p className={c("topic-panel-label")}>
                  冻结材料包 <small>FROZEN MATERIAL PACK</small>
                </p>
                <h2 id="topic-materials-title">精确 Work Resource 引用</h2>
              </div>
              <span id="topic-material-count">
                {data ? visible.length : "—"}
              </span>
            </header>
            <div
              className={c("topic-feedback")}
              id="topic-feedback"
              data-state={
                query.isError ? "unavailable" : data ? "ready" : undefined
              }
              role="status"
              aria-live="polite"
            >
              {feedback}
            </div>
            <div
              className={c("topic-material-list")}
              id="topic-material-list"
              role="listbox"
              aria-label="Topic 材料"
            >
              {visible.map((item) => (
                <button
                  type="button"
                  key={item.ref}
                  className={c("topic-material")}
                  data-role={item.role}
                  data-ref={item.ref}
                  role="option"
                  aria-selected={item.ref === material?.ref}
                  onClick={() => setSelected(item.ref)}
                >
                  <span className={c("topic-material-index")}>
                    {String((data?.materials.indexOf(item) ?? 0) + 1).padStart(
                      2,
                      "0",
                    )}
                  </span>
                  <span className={c("topic-material-main")}>
                    <strong className={c("topic-material-title")}>
                      {item.title ?? "标题未知"}
                    </strong>
                    <span
                      className={c("topic-material-meta")}
                    >{`${item.platform} · ${item.externalId} · ${item.creator ?? "创作者未知"}`}</span>
                  </span>
                  <span className={c("topic-role")} data-role={item.role}>
                    {label(item.role)}材料
                  </span>
                </button>
              ))}
              {data && visible.length === 0 ? (
                <p className={c("topic-feedback")}>
                  当前{machine ? "分类" : "裁定"}视角没有材料；这不代表来源世界中不存在相关内容。
                </p>
              ) : null}
            </div>
          </section>
          <Inspector material={material} machine={machine} />
        </section>
        <section className={c("topic-boundary")} id="boundary">
          <div>
            <p className={c("topic-panel-label")}>
              来源边界 <small>SOURCE BOUNDARY</small>
            </p>
            <p id="topic-source-boundary">
              {topic?.sourceBoundary ??
                "尚未读取；不能判断覆盖、代表性或新鲜度。"}
            </p>
          </div>
          <div>
            <strong>这里没有宣称</strong>
            <p>
              没有正式 Topic 发布、趋势结论、市场规模、Claim 或自动分类成功。
            </p>
          </div>
        </section>
      </main>
    </div>
  );
}
export function WorkspacePage() {
  const { canonical_key = "" } = useParams();
  const query = useWorkspace(canonical_key);
  const topic = query.isSuccess && !query.isFetching ? query.data.topic : undefined;
  useEffect(() => {
    document.title = "Topic 工作区 · Linggan Intelligence";
  }, []);
  return (
    <AppShell
      className={c("topic-app")}
      active="topic"
      boundaryLabel="本机服务状态"
      crumb={
        <>
          主题图谱 <span className="v7-slash">/</span> <b>领域探索</b>{" "}
          <span className="v7-slash">/</span>{" "}
          <span className="v7-context-current">Topic 工作区</span>
        </>
      }
      meta={
        <>
          <span className="v7-kpi">
            <em>生命周期</em>
            <b>{topic ? topic.definition.lifecycleState === "candidate" ? "候选" : "暂定" : "未读取"}</b>
          </span>
          <span className="v7-kpi">
            <em>裁定方式</em>
            <b>{topic ? topic.runKind === "machine_proposed" ? "机器候选" : "人工" : "未读取"}</b>
          </span>
          <i className="v7-vr" aria-hidden="true" />
          <span>
            来源与覆盖不得外推{" "}
            <span className="v7-tech-key">NO GENERALIZATION</span>
          </span>
        </>
      }
    >
      <WorkspaceContents key={canonical_key} canonicalKey={canonical_key} query={query} />
    </AppShell>
  );
}
