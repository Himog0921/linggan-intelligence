import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link, useLocation, useSearchParams } from "react-router";
import { AppShell } from "../../app/AppShell";
import * as api from "./api";
import { Status } from "./ui";
import { c } from "./styles";
import { LedgerPage } from "./LedgerPage";
import { OverviewPage } from "./OverviewPage";
const entries = [
  ["overview", "总览"],
  ["publications", "发布记录"],
  ["accounts", "账号管理"],
  ["daily", "每日数据"],
  ["io", "导入导出"],
] as const;
export type LedgerKind = "publications" | "accounts" | "daily";
export function OpsPage() {
  const location = useLocation();
  const [params, setParams] = useSearchParams();
  const business = params.get("domain") || "all";
  const current =
    entries.find(([name]) => location.pathname === `/ops/${name}`) ||
    entries[0];
  useEffect(() => {
    document.title = `${current[1]} · 运营`;
  }, [current]);
  const options = useQuery({
    queryKey: ["ops", "businesses"],
    queryFn: ({ signal }) =>
      api.get(api.url("businesses"), api.businesses, signal),
  });
  const invalid =
    options.data !== undefined &&
    business !== "all" &&
    !options.data.some((o) => o.domainRef === business);
  useEffect(() => {
    if (invalid) setParams({ domain: "all" }, { replace: true });
    else if (!params.get("domain")) {
      const next = new URLSearchParams(params);
      next.set("domain", "all");
      setParams(next, { replace: true });
    }
  }, [invalid, params, setParams]);
  const selected = options.data?.find((o) => o.domainRef === business);
  const summary = useQuery({
    queryKey: ["ops", "summary", business],
    queryFn: ({ signal }) =>
      api.get(api.url("summary", { domain: business }), api.summary, signal),
    enabled: !invalid && options.isSuccess,
  });
  const foot = summary.data?.asOf
    ? `数据截至 ${summary.data.asOf.exportedAtBeijing.slice(5)}`
    : summary.isPending || summary.isError
      ? "数据截至时间未知"
      : "尚未迁入数据";
  return (
    <AppShell
      active="ops"
      boundaryLabel="本机服务状态"
      crumb={
        <>
          运营 /{" "}
          {options.data ? (
            <details
              key={`${current[0]}:${business}`}
              className="v7-domain-picker"
              onClick={(event) => {
                if (
                  event.target instanceof Element &&
                  event.target.closest("a")
                )
                  event.currentTarget.open = false;
              }}
            >
              <summary aria-label="切换业务">
                <span>{selected?.name || "全部业务"}</span>
                <small>
                  {selected ? `${selected.accountCount} 个账号` : "全部账号"}
                </small>
                <i aria-hidden="true" />
              </summary>
              <nav aria-label="可选业务">
                <Link
                  to={`/ops/${current[0]}?domain=all`}
                  aria-current={business === "all" ? "page" : undefined}
                  aria-label="全部业务"
                >
                  <span>全部业务</span>
                  <small>全部账号</small>
                </Link>
                {options.data.map((o) => (
                  <Link
                    key={o.domainRef}
                    to={`/ops/${current[0]}?domain=${o.domainRef}`}
                    aria-current={business === o.domainRef ? "page" : undefined}
                    aria-label={`${o.name}，${o.accountCount} 个账号`}
                  >
                    <span>{o.name}</span>
                    <small>{o.accountCount} 个账号</small>
                  </Link>
                ))}
              </nav>
            </details>
          ) : null}{" "}
          / <b>{current[1]}</b>
        </>
      }
      meta={
        <>
          <span>只读</span>
          <span>迁入数据</span>
        </>
      }
    >
      <div className="v7-shell">
        <aside className="v7-side" aria-label="运营导航">
          {entries.map(([name, title], i) => (
            <Link
              key={name}
              className="v7-side-nav"
              to={`/ops/${name}?domain=${encodeURIComponent(business)}`}
              aria-current={current[0] === name ? "page" : undefined}
            >
              <i>{String(i + 1).padStart(2, "0")}</i>
              <span>{title}</span>
            </Link>
          ))}
          <div className="v7-side-foot">{foot}</div>
        </aside>
        <main className={c("ops-main")} aria-labelledby="ops-title">
          <h1 id="ops-title" className="v7-sr-only">
            {current[1]}
          </h1>
          {options.isError ? (
            <Status error retry={() => void options.refetch()}>
              业务读取失败（{api.reason(options.error)}），可以重试。
            </Status>
          ) : invalid || options.isPending ? (
            <Status>正在读取业务…</Status>
          ) : current[0] === "overview" ? (
            <OverviewPage key={`overview:${business}`} business={business} />
          ) : current[0] === "io" ? (
            <ImportExport key={`io:${business}`} business={business} />
          ) : (
            <LedgerPage
              key={`${current[0]}:${business}`}
              kind={current[0]}
              business={business}
              summary={summary}
            />
          )}
        </main>
      </div>
    </AppShell>
  );
}
function ImportExport({ business }: { business: string }) {
  return (
    <div className={c("ops-io")}>
      <section className={c("ops-panel")} aria-labelledby="ops-export-title">
        <h2 id="ops-export-title">导出</h2>
        <p className={c("ops-note")}>
          导出 Linggan
          里当前业务的全部记录（每日数据不分业务），列与页面一致，附来源记录号和数据截至时间。
        </p>
        <ul className={c("ops-export-list")}>
          {[
            ["accounts", "账号"],
            ["publications", "发布记录"],
            ["daily", "每日数据"],
          ].map(([kind, name]) => (
            <li key={kind}>
              <span>{name}</span>
              {["csv", "json"].map((format) => (
                <a
                  key={format}
                  href={api.url("export", {
                    domain: business,
                    kind: kind || "",
                    format,
                  })}
                >
                  {format.toUpperCase()}
                </a>
              ))}
            </li>
          ))}
        </ul>
      </section>
      <section
        className={c("ops-panel")}
        aria-labelledby="ops-import-title"
        aria-disabled="true"
      >
        <h2 id="ops-import-title">导入</h2>
        <p className={c("ops-note")}>
          导入随录入功能在登录上线后开放；切换前请在内容工作台导入。
        </p>
        <label>
          导入类型
          <select disabled>
            <option>发布记录</option>
          </select>
        </label>
        <label>
          导入内容
          <textarea disabled rows={6} />
        </label>
        <button type="button" disabled>
          导入 CSV
        </button>
      </section>
    </div>
  );
}
