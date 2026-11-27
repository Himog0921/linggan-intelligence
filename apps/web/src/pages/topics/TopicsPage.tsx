import { useNavigate } from "react-router";
import { useQuery } from "@tanstack/react-query";
import {
  Fragment,
  memo,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type RefObject,
} from "react";
import { AppShell } from "../../app/AppShell";
import { ContextBreadcrumb } from "../../shared/ContextBar";
import { DomainScopePicker } from "../../shared/DomainScopePicker";
import { requestJson } from "../../shared/api";
import * as d from "../../shared/decode";
import { createTopicEngine } from "./engine";
import { TopicMutationContext } from "./topicMutationContext";
import { c } from "./styles";

function context(value: unknown) {
  return d.decode(value, (root) => ({
    defaultDomainRef:
      d.field(root, "defaultDomainRef") === null
        ? null
        : d.str(root, "defaultDomainRef"),
    domains: d.list(root, "domains", (domain) => ({
      domainRef: d.str(domain, "domainRef"),
      name: d.str(domain, "name"),
      status: d.str(domain, "status"),
      targetCount: d.num(domain, "targetCount"),
      sampleCount:
        d.field(domain, "sampleCount") === null
          ? null
          : d.num(domain, "sampleCount"),
    })),
  }));
}
const TopicContents = memo(function TopicContents({
  engine,
  mainVersion,
  notice,
  error,
}: {
  engine: ReturnType<typeof createTopicEngine>;
  mainVersion: number;
  notice: string;
  error: string | null;
}) {
  return (
    <div className={c("lgi-tm-content")}>
      <Fragment key={mainVersion}>
        {error ? (
          <p role="alert">领域读取未成功：{error}</p>
        ) : (
          engine.getContent(notice)
        )}
      </Fragment>
    </div>
  );
});
/** Topic tree aids that the engine's string-built markup cannot own: a full-name peek for names cut by
 * the 224px column, and arrow-key movement between visible topics (Left/Right fold or climb). */
function useTreeNavigation(root: RefObject<HTMLDivElement | null>) {
  const [peek, setPeek] = useState<{ name: string; meta: string; top: number; left: number } | null>(null);
  useEffect(() => {
    const node = root.current;
    if (!node) return;
    const items = () =>
      [...node.querySelectorAll<HTMLButtonElement>("[data-tree] [data-tree-item]")].filter((el) => el.offsetParent !== null);
    const show = (el: HTMLElement) => {
      if (el.scrollWidth <= el.clientWidth + 1) return setPeek(null);
      const row = el.getBoundingClientRect();
      const column = el.closest("aside")?.getBoundingClientRect();
      setPeek({
        name: el.getAttribute("title") || el.textContent || "",
        meta: el.dataset.peek || "",
        top: row.top + row.height / 2,
        left: (column?.right ?? row.right) + 10,
      });
    };
    const over = (event: Event) => {
      const el = event.target instanceof Element ? event.target.closest<HTMLElement>("[data-tree] [data-tree-item]") : null;
      if (el) show(el);
      else setPeek(null);
    };
    const focus = (event: FocusEvent) => {
      const el = event.target instanceof HTMLElement ? event.target.closest<HTMLElement>("[data-tree] [data-tree-item]") : null;
      if (el && el.matches(":focus-visible")) show(el);
      else setPeek(null);
    };
    const hide = () => setPeek(null);
    const key = (event: KeyboardEvent) => {
      const el = event.target instanceof HTMLElement ? event.target.closest<HTMLButtonElement>("[data-tree] [data-tree-item]") : null;
      if (!el || event.altKey || event.ctrlKey || event.metaKey) return;
      const list = items(), i = list.indexOf(el);
      const toggle = el.parentElement?.querySelector<HTMLButtonElement>("[data-action=\"collapse\"]");
      const go = (target: HTMLButtonElement | undefined) => {
        if (!target) return;
        event.preventDefault();
        target.focus();
        target.scrollIntoView({ block: "nearest" });
      };
      if (event.key === "ArrowDown") go(list[Math.min(i + 1, list.length - 1)]);
      else if (event.key === "ArrowUp") go(list[Math.max(i - 1, 0)]);
      else if (event.key === "Home") go(list[0]);
      else if (event.key === "End") go(list[list.length - 1]);
      else if (event.key === "ArrowRight") {
        event.preventDefault();
        if (toggle?.getAttribute("aria-expanded") === "false") toggle.click();
        else if (toggle) go(list[i + 1]);
      } else if (event.key === "ArrowLeft") {
        event.preventDefault();
        if (toggle?.getAttribute("aria-expanded") === "true") toggle.click();
        else if (el.dataset.parentRef) go(list.find((x) => x.dataset.id === el.dataset.parentRef));
      }
    };
    node.addEventListener("pointerover", over);
    node.addEventListener("pointerleave", hide);
    node.addEventListener("focusin", focus);
    node.addEventListener("keydown", key);
    node.addEventListener("scroll", hide, true);
    return () => {
      node.removeEventListener("pointerover", over);
      node.removeEventListener("pointerleave", hide);
      node.removeEventListener("focusin", focus);
      node.removeEventListener("keydown", key);
      node.removeEventListener("scroll", hide, true);
    };
  }, [root]);
  return peek;
}

export function TopicsPage() {
  const navigate = useNavigate();
  const [engine] = useState(() => createTopicEngine(path => { void navigate(path); }));
  const version = useSyncExternalStore(engine.subscribe, engine.getVersion);
  const root = useRef<HTMLDivElement>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const peek = useTreeNavigation(root);
  const state = engine.getState();
  const currentDialog = engine.getDialog();
  const shell = useQuery({
    queryKey: ["topics", "pageContext"],
    queryFn: async ({ signal }) =>
      context(await requestJson("/api/local/topic-map/context", { signal })),
  });
  const domain = shell.data?.domains.find(
    (domain) => domain.domainRef === state.domainRef,
  );
  useEffect(() => {
    document.title = "主题图谱 · Linggan Intelligence";
    const cls = c("topic-map-page");
    document.body.classList.add(cls);
    window.addEventListener("popstate", engine.onLocation);
    return () => {
      window.removeEventListener("popstate", engine.onLocation);
      document.body.classList.remove(cls);
      engine.dispose();
    };
  }, [engine]);
  useEffect(() => {
    if (shell.data) void engine.activate(shell.data.defaultDomainRef || "");
  }, [engine, shell.data]);
  useLayoutEffect(() => {
    engine.attachRoot(root.current);
    engine.attachDialog(dialog.current);
    if (currentDialog && dialog.current && !dialog.current.open)
      dialog.current.showModal();
    const scroll = engine.consumeDialogScroll();
    if (scroll !== null) {
      const body = dialog.current?.querySelector("[data-topic-dialog-body]");
      if (body) body.scrollTop = scroll;
    }
    const mainScroll = engine.consumeMainScroll();
    if (mainScroll !== null) {
      const main = root.current?.querySelector("[data-topic-main]");
      if (main) main.scrollTop = mainScroll;
    }
    const findLabel = engine.consumeFindFocus();
    if (findLabel) {
      const input = [...(root.current?.querySelectorAll<HTMLInputElement>("[data-find] input") ?? [])].find(
        (node) => node.getAttribute("aria-label") === findLabel,
      );
      if (input && document.activeElement !== input) {
        input.focus({ preventScroll: true });
        input.setSelectionRange(input.value.length, input.value.length);
      }
    }
    if (engine.consumeScrollReset())
      root.current
        ?.querySelector('[aria-label="主题内容"]')
        ?.scrollTo({ top: 0 });
  }, [engine, version, currentDialog]);
  return (
    <TopicMutationContext value={engine.isMutationBusy()}>
      <AppShell
        className={c("topic-map-app")}
        active="topic"
        boundaryLabel="本机不访问平台"
        contextBar={{
        breadcrumb: (
          <>
            <ContextBreadcrumb items={["主题图谱"]} />
            {shell.data ? (
              <DomainScopePicker
                label="切换当前观察领域"
                summary={
                  <>
                    <span>{domain?.name || "选择领域"}</span>
                    <small>
                      {domain
                        ? domain.status === "paused"
                          ? "已暂停"
                          : domain.targetCount + " 个目标 · " + (domain.sampleCount ?? "未知") + " 篇作品"
                        : "请选择一个领域"}
                    </small>
                    <i aria-hidden="true" />
                  </>
                }
              >
                <nav aria-label="可选观察领域">
                  {shell.data.domains.map((item) => (
                    <a
                      key={item.domainRef}
                      href={"/topics?domain=" + item.domainRef}
                      aria-current={state.domainRef === item.domainRef ? "page" : undefined}
                    >
                      <span>{item.name}</span>
                      <small>
                        {item.status === "paused"
                          ? "已暂停"
                          : item.targetCount + " 个目标 · " + (item.sampleCount ?? "未知") + " 篇作品"}
                      </small>
                    </a>
                  ))}
                </nav>
              </DomainScopePicker>
            ) : null}
          </>
        ),
        meta: (
          <div id="topic-map-context-actions" className="v7-context-actions" aria-label="图谱操作" onClick={engine.onClick}>
            {engine.getContextActions()}
          </div>
        ),
      }}>
        <div
          id="topic-map-root"
          data-view={state.view}
          ref={root}
          aria-label="主题图谱"
          onClick={engine.onClick}
          onChange={engine.onChange}
          onSubmit={engine.onSubmit}
          onKeyDown={engine.onKeyDown}
          onToggle={engine.onToggle}
        >
          <TopicContents
            engine={engine}
            mainVersion={engine.getMainVersion()}
            notice={engine.getNotice()}
            error={shell.isError ? shell.error.message : null}
          />
          {peek ? (
            <div
              className={c("lgi-tm-peek")}
              role="tooltip"
              style={{ top: peek.top, left: peek.left }}
            >
              {peek.name}
              {peek.meta ? <small>{peek.meta}</small> : null}
            </div>
          ) : null}
          {currentDialog ? (
            <dialog
              ref={dialog}
              data-kind={currentDialog.type}
              className={c("lgi-tm-dialog")}
              aria-labelledby="topic-map-dialog-title"
              onCancel={(event) => {
                event.preventDefault();
                engine.closeDialog();
              }}
              onClick={(event) => {
                event.stopPropagation();
                if (event.target === event.currentTarget) engine.closeDialog();
                else void engine.onClick(event);
              }}
              onSubmit={(event) => {
                event.stopPropagation();
                engine.onSubmit(event);
              }}
            >
              {engine.getDialogContent()}
            </dialog>
          ) : null}
        </div>
      </AppShell>
    </TopicMutationContext>
  );
}
