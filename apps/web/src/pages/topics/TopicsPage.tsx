import { useQuery } from "@tanstack/react-query";
import {
  Fragment,
  memo,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { AppShell } from "../../app/AppShell";
import { ContextBreadcrumb } from "../../shared/ContextBar";
import { ContextTabs } from "../../shared/ContextTabs";
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
export function TopicsPage() {
  const [engine] = useState(createTopicEngine);
  const version = useSyncExternalStore(engine.subscribe, engine.getVersion);
  const root = useRef<HTMLDivElement>(null);
  const dialog = useRef<HTMLDialogElement>(null);
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
          <ContextBreadcrumb
            items={[
              "主题图谱",
              ...(shell.data ? [
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
                </DomainScopePicker>,
              ] : []),
              state.view === "journey" ? "用户生命旅程" : "主题概览",
            ]}
          />
        ),
        navigation: (
          <ContextTabs id="topic-map-view-tabs" label="主题图谱视图" onClick={engine.onClick}>
            {engine.getViewTabs()}
          </ContextTabs>
        ),
        meta: (
          <div id="topic-map-context-actions" className="v7-context-actions" aria-label="图谱操作" onClick={engine.onClick}>
            <a className="v7-context-action" href="/topics/offline-review">离线预演</a>
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
