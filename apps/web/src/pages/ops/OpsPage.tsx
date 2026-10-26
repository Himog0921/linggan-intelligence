import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { Navigate, useLocation, useSearchParams } from "react-router";
import { OpsFrame } from "./OpsFrame";
import { OpsScopePicker } from "./OpsScopePicker";
import * as api from "./api";
import { Status } from "./ui";
import { c } from "./styles";
import { LedgerPage } from "./LedgerPage";
import { OverviewPage } from "./OverviewPage";
import { requestJson } from "../../shared/api";
const entries = [['overview', '概览'], ['publications', '发布记录'], ['accounts', '账号管理'], ['daily', '每日数据']] as const;
export type LedgerKind = "publications" | "accounts" | "daily";
export function OpsPage() {
  const location = useLocation();
  if (location.pathname === "/ops/io") return <Navigate replace to={`/ops/overview${location.search}`} />;
  return <OpsDataPage />;
}
function OpsDataPage() {
  const session = useQuery({
    queryKey: ['auth', 'session'],
    queryFn: ({ signal }) => requestJson('/api/local/auth/session', { signal }),
  });
  const role = (session.data as { member?: { role?: string } } | undefined)?.member?.role;
  const displayName = (session.data as { member?: { displayName?: string } } | undefined)?.member?.displayName;
  const restricted = role !== 'admin';
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
    if (invalid) {
      const next = new URLSearchParams(params);
      next.set("domain", "all"); next.delete("page");
      setParams(next, { replace: true });
    }
    else if (!params.get("domain")) {
      const next = new URLSearchParams(params);
      next.set("domain", "all");
      setParams(next, { replace: true });
    }
  }, [invalid, params, setParams]);
  const summary = useQuery({
    queryKey: ["ops", "summary", business],
    queryFn: ({ signal }) =>
      api.get(api.url("summary", { domain: business }), api.summary, signal),
    enabled: !invalid && options.isSuccess,
  });
  const foot = summary.data?.asOf
    ? `数据截至 ${summary.data.asOf.exportedAtBeijing.slice(5)}`
    : summary.data?.nativeDataPresent
      ? "原生台账数据（历史尚未迁入）"
    : summary.isPending || summary.isError
      ? "数据截至时间未知"
      : "尚未迁入数据";
  if (session.isPending) return <p role="status">正在核对团队身份…</p>;
  if (session.isError) return <p role="alert">团队身份读取失败，请刷新页面后重试。</p>;
  return (
    <OpsFrame section="data" domain={business} foot={foot} restricted={restricted}
      scope={<OpsScopePicker options={options.data ?? []} value={business} loading={options.isPending} failed={options.isError}
        onChange={domain => setParams({ domain })} />}
      tabs={(restricted ? entries.slice(1) : entries).map(([name, title]) => <a key={name} href={`/ops/${name}?domain=${encodeURIComponent(business)}`}
        aria-current={current[0] === name ? 'page' : undefined}>{title}</a>)}
      meta={<><span>可录入台账</span><span>历史迁入数据只读</span></>}>
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
          ) : (
            <LedgerPage
              key={`${current[0]}:${business}`}
              kind={current[0]}
              business={business}
              restricted={restricted}
              displayName={displayName ?? ''}
              role={role ?? ''}
            />
          )}
        </main>
    </OpsFrame>
  );
}
