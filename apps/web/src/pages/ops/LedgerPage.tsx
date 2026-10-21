import { useOpsLeave } from './useOpsLeave';
import { ActionButton } from '../../shared/ActionButton';
import { FindField } from '../../shared/FindField';
import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useSearchParams } from "react-router";
import * as api from "./api";
import * as f from "./format";
import {
  Dialog,
  DisabledActions,
  Empty,
  External,
  Missing,
  NumberCell,
  Status,
  Tag,
  Tabs,
  TextCell,
} from "./ui";
import { c, peopleTabs, readonly } from "./styles";
import type { LedgerKind } from "./OpsPage";
const columns = {
  publications: [
    "日期",
    "平台",
    "标题",
    "数据状态",
    "发布链接",
    "媒体",
    "正文",
    "账号",
    "运营人",
    "操作",
    "阅读量",
    "点赞",
    "收藏",
    "评论",
    "封面点击率",
    "复盘",
  ],
  accounts: [
    "序号",
    "平台",
    "账号名称",
    "设备编号",
    "粉丝",
    "IP所在地",
    "运营人",
    "账号状态",
    "账号健康",
    "保留",
    "账号备注",
    "操作",
  ],
  daily: [
    "日期",
    "星期",
    "运营人",
    "执行状态",
    "发布",
    "打开",
    "加微",
    "添加率",
    "爆款",
    "月累计加微",
    "月累计发布",
    "操作",
  ],
};
const colNames = {
  publications: [
    "date",
    "platform",
    "title",
    "state",
    "link",
    "media",
    "body",
    "account",
    "person",
    "actions",
    "number",
    "number",
    "number",
    "number",
    "number",
    "review",
  ],
  accounts: [
    "ordinal",
    "platform",
    "title",
    "code",
    "number",
    "code",
    "person",
    "state",
    "state",
    "flag",
    "body",
    "actions",
  ],
  daily: [
    "date",
    "platform",
    "person",
    "flag",
    "number",
    "number",
    "number",
    "number",
    "number",
    "number",
    "number",
    "actions",
  ],
};
const numeric = {
  publications: [10, 11, 12, 13, 14],
  accounts: [0, 4],
  daily: [4, 5, 6, 7, 8, 9, 10],
};
const titles: Record<string, string> = {
  账号健康: "改为运营手动填写，录入功能上线后开放",
  执行状态:
    "未完成：当天没有发布登记；可表扬：有爆款或有加微；待补阅读量：当天的小红书帖子满 7 天还没填阅读量",
  发布: "该运营人当天的发布登记条数，重复登记只算一次",
  打开: "工作台原值；工作台默认填 0，0 和没填分不开",
  加微: "工作台原值；工作台默认填 0，0 和没填分不开",
  添加率: "加微 ÷ 打开",
  爆款: "当天发布的小红书帖子中，最新阅读量不少于 5000 的篇数",
};
export function LedgerPage({
  kind,
  business,
}: {
  kind: LedgerKind;
  business: string;
}) {
  const [params, setParams] = useSearchParams();
  const operator = params.get("operator") || "";
  const [dates, setDates] = useState(false);
  const [cover, setCover] = useState<string | null>(null);
  const [coverFailed, setCoverFailed] = useState(false);
  const [detail, setDetail] = useState<api.Publication | null>(null);
  function openCover(src: string) { setCoverFailed(false); setCover(src); }
  const wrap = useRef<HTMLDivElement>(null);
  const query = Object.fromEntries(params);
  const pubs = useQuery({
    queryKey: ["ops", "publications", business, params.toString()],
    queryFn: ({ signal }) =>
      api.get(
        api.url("publications", { ...query, domain: business }),
        api.publications,
        signal,
      ),
    enabled: kind === "publications",
  });
  const accounts = useQuery({
    queryKey: ["ops", "accounts", business, operator],
    queryFn: ({ signal }) =>
      api.get(
        api.url("accounts", { domain: business, operator }),
        api.accounts,
        signal,
      ),
    enabled: kind === "accounts",
  });
  const daily = useQuery({
    queryKey: ["ops", "daily", business, operator],
    queryFn: ({ signal }) =>
      api.get(
        api.url("daily", { domain: business, operator }),
        api.daily,
        signal,
      ),
    enabled: kind === "daily",
  });
  const result =
    kind === "publications" ? pubs : kind === "accounts" ? accounts : daily;
  const data = result.data;
  const filtered = [
    "operator",
    "platform",
    "account",
    "from",
    "to",
    "views",
  ].some((k) => params.get(k));
  function update(values: Record<string, string>, page = false) {
    const next = new URLSearchParams(params);
    for (const [k, v] of Object.entries(values)) {
      if (v) next.set(k, v);
      else next.delete(k);
    }
    if (!page) next.delete("page");
    setParams(next, { replace: true });
    if (wrap.current) {
      wrap.current.scrollTop = 0;
    }
  }
  useEffect(() => {
    if (
      pubs.data &&
      pubs.data.items.length === 0 &&
      pubs.data.total > 0 &&
      pubs.data.page > 1
    ) {
      const next = new URLSearchParams(params);
      next.delete("page");
      setParams(next, { replace: true });
    }
  }, [pubs.data, params, setParams]);
  const failures = [
    result.isError
      ? `${kind === "publications" ? "发布记录" : kind === "accounts" ? "账号" : "每日数据"}（${api.reason(result.error)}）`
      : "",
  ].filter(Boolean);
  const operators = data?.operators || (operator ? [operator] : []);
  return (
    <>
      <div
        className={c(
          "ops-toolbar",
          ...(kind === "publications" ? ["ops-toolbar-stacked"] : []),
        )}
      >
        <div className={c("ops-tools")}>
          {kind === "publications" ? (
            <>
              <FindField formId="ops-account-search" inputId="ops-account" name="account" className={c('ops-find')}
                label="搜索账号" value={params.get('account') || ''} placeholder="账号名称" maxLength={200}
                contextKey={business} trigger="submit" onCommit={account => update({ account })} shortcut submitLabel="搜索" />
              <nav className={c("ops-tabs", "ops-platforms")} aria-label="平台">
                {[
                  ["", "总数据"],
                  ["douyin", "抖音"],
                  ["xhs", "小红书"],
                ].map(([value = "", text]) => (
                  <button
                    type="button"
                    key={value}
                    aria-pressed={(params.get("platform") || "") === value}
                    onClick={() => update({ platform: value })}
                  >
                    {text}
                  </button>
                ))}
              </nav>


              <ActionButton
                type="button"
                id="ops-date-open"
                onClick={() => setDates(true)}
              >
                {params.get("from") || params.get("to")
                  ? `${params.get("from") || "开始"} ~ ${params.get("to") || "结束"}`
                  : "日期范围"}
              </ActionButton>
              <ActionButton
                type="button"
                id="ops-pending"
                aria-pressed={params.get("views") === "pending"}
                onClick={() =>
                  update({
                    views: params.get("views") === "pending" ? "" : "pending",
                  })
                }
              >
                待补阅读量
              </ActionButton>
            </>
          ) : kind === "accounts" ? (
            <span
              id="ops-account-counts"
              className={c("ops-counts")}
              aria-live="polite"
            >
              {accounts.data
                ? `总账号 ${accounts.data.total} · 运营中 ${accounts.data.active}`
                : ""}
            </span>
          ) : (
            <span className={c("ops-counts")}>
              迁入自内容工作台 · 发布数按发布记录自动统计
            </span>
          )}
          <small className={c("ops-note")}>切换前只读</small>
          {kind === "publications" ? (
            <>
              <ActionButton type="button" disabled title={readonly}>
                新增发布
              </ActionButton>
              <ActionButton type="button" disabled title="自动发现我方帖子上线后可用">
                同步数据
              </ActionButton>
            </>
          ) : kind === "accounts" ? (
            <ActionButton type="button" disabled title={readonly}>
              新增账号
            </ActionButton>
          ) : null}
        </div>
        <Tabs
          id="ops-operators"
          label="运营人"
          values={peopleTabs(operators)}
          current={operator}
          onSelect={(name) => update({ operator: name })}
        />
      </div>
      {kind === "daily" ? (
        <p
          id="ops-scope-note"
          className={c("ops-scope-note")}
          hidden={business === "all"}
        >
          每日数据不分业务，下面是全部业务的数据。
        </p>
      ) : null}
      <Status
        error={failures.length > 0}
        retry={
          failures.length
            ? () => {
                void result.refetch();
              }
            : undefined
        }
      >
        {failures.length
          ? `${failures.join("、")}读取失败，可以重试；筛选条件保持不变。`
          : null}
      </Status>
      <div
        ref={wrap}
        data-ops-table
        className={c("ops-table-wrap")}
        tabIndex={0}
        aria-label={`${kind === "publications" ? "发布记录" : kind === "accounts" ? "账号" : "每日数据"}表格，可横向滚动`}
        aria-busy={result.isFetching}
      >
        <table className={c("ops-table", `ops-${kind}`)}>
          <colgroup>
            {colNames[kind].map((name, i) => (
              <col key={i} className={c(`ops-col-${name}`)} />
            ))}
          </colgroup>
          <thead>
            <tr>
              {columns[kind].map((title, i) => (
                <th
                  key={title}
                  scope="col"
                  className={
                    numeric[kind].includes(i) ? c("ops-number") : undefined
                  }
                  title={titles[title]}
                >
                  {title}
                </th>
              ))}
            </tr>
          </thead>
          <tbody id="ops-rows">
            {result.isError ? null : kind === "publications" &&
              pubs.data?.items.length ? (
              pubs.data.items.map((item) => (
                <PublicationRow
                  key={item.ref}
                  item={item}
                  openCover={openCover}
                  openDetail={setDetail}
                />
              ))
            ) : kind === "accounts" && accounts.data?.items.length ? (
              accounts.data.items.map((item, i) => (
                <AccountRow key={item.ref} item={item} index={i} />
              ))
            ) : kind === "daily" && daily.data?.items.length ? (
              daily.data.items.map((item) => (
                <DailyRow key={item.ref} item={item} />
              ))
            ) : data && !result.isFetching ? (
              <Empty
                columns={columns[kind].length}
                text={
                  !data.asOf
                    ? "还没有迁入数据。"
                    : kind === "publications"
                      ? filtered
                        ? "没有符合条件的发布记录。"
                        : "没有发布记录。"
                      : kind === "accounts"
                        ? "当前筛选下没有账号。"
                        : "当前筛选下没有每日数据。"
                }
                action={
                  kind === "publications" && filtered ? (
                    <ActionButton
                      type="button"
                      onClick={() => {
                        update({
                          operator: "",
                          platform: "",
                          account: "",
                          from: "",
                          to: "",
                          views: "",
                        });
                      }}
                    >
                      清除筛选
                    </ActionButton>
                  ) : null
                }
              />
            ) : null}
          </tbody>
        </table>
      </div>
      {kind === "publications" ? (
        <nav className={c("ops-pagination")} aria-label="发布记录分页">
          <span id="ops-page-summary" aria-live="polite">
            {pubs.data && !pubs.isError
              ? `第 ${pubs.data.page} / ${Math.max(pubs.data.totalPages, 1)} 页 · 共 ${pubs.data.total.toLocaleString("zh-CN")} 条 · 每页最多 ${pubs.data.pageSize} 条`
              : ""}
          </span>
          <ActionButton
            type="button"
            id="ops-prev"
            disabled={pubs.isFetching || !pubs.data || pubs.data.page <= 1}
            onClick={() =>
              update({ page: String((pubs.data?.page || 1) - 1) }, true)
            }
          >
            上一页
          </ActionButton>
          <ActionButton
            type="button"
            id="ops-next"
            disabled={
              pubs.isFetching ||
              !pubs.data ||
              pubs.data.page >= Math.max(pubs.data.totalPages, 1)
            }
            onClick={() =>
              update({ page: String((pubs.data?.page || 1) + 1) }, true)
            }
          >
            下一页
          </ActionButton>
        </nav>
      ) : (
        <p className={c("ops-footnote")}>
          {kind === "accounts"
            ? "只列在台账的账号；已在工作台删除的账号只出现在发布记录里，标「已停用」。账号健康改为运营手动填写，录入功能上线后开放。"
            : "显示最近 200 行。打开、加微按工作台原值显示；工作台里这两列默认是 0，0 和没填分不开。"}
        </p>
      )}
      {dates ? (
        <DateDialog
          from={params.get("from") || ""}
          to={params.get("to") || ""}
          close={() => setDates(false)}
          save={(from, to) => {
            update({ from, to });
            setDates(false);
          }}
        />
      ) : null}
      {detail ? <Dialog label="发布记录详情" outsideClose onClose={() => setDetail(null)}>
        <header className={c('ops-detail-head')}><h2>{detail.title || '无标题发布记录'}</h2><ActionButton onClick={() => setDetail(null)}>关闭详情</ActionButton></header>
        <div className={c('ops-dialog-body')}>
          <p>{detail.businessDate} · {f.platformLabel[detail.platform]} · {detail.account?.name || '账号未知'}</p>
          <p className={c('ops-note')}>迁入记录 · 系统尚未核对平台发布状态</p>
          <External href={detail.url} title="原发布链接">查看原发布链接 ↗</External>
          <dl className={c('ops-detail-facts')}>
            <div><dt>阅读量</dt><dd>{detail.views.state === 'value' ? f.count(detail.views.value).text : detail.views.state === 'pending' ? '待补' : '选填，未知'}</dd></div>
            <div><dt>点赞</dt><dd>{f.count(detail.likes).text}</dd></div><div><dt>收藏</dt><dd>{f.count(detail.collects).text}</dd></div><div><dt>评论</dt><dd>{f.count(detail.comments).text}</dd></div>
          </dl>
          <h3>发布正文</h3><p className={c('ops-full-text')}>{detail.body || '未迁入正文'}</p>
          {detail.duplicates.length ? <p>合并的重复登记日期：{detail.duplicates.map(row => row.businessDate).join('、')}</p> : null}
          {detail.cover.available ? <ActionButton onClick={() => openCover(`/ops/covers/${encodeURIComponent(detail.ref)}`)}>查看封面大图</ActionButton> : <p>封面未迁入</p>}
        </div>
      </Dialog> : null}
      {cover ? (
        <Dialog
          label="封面大图" outsideClose
          className="ops-cover-dialog"
          onClose={() => setCover(null)}
        >
          <form
            method="dialog"
            onSubmit={(event) => {
              event.preventDefault();
              setCover(null);
            }}
          >
            <ActionButton type="submit">关闭</ActionButton>
          </form>
          {coverFailed ? <div role="status"><p>封面暂时无法读取，原记录仍保留。</p><ActionButton onClick={() => setCoverFailed(false)}>重新加载封面</ActionButton></div> : <img id="ops-cover-large" src={cover} alt="封面大图" onError={() => setCoverFailed(true)} />}
        </Dialog>
      ) : null}
    </>
  );
}
function PublicationRow({
  item,
  openCover, openDetail,
}: {
  item: api.Publication;
  openCover: (src: string) => void;
  openDetail: (item: api.Publication) => void;
}) {
  return (
    <tr>
      <td>
        <div className={c("ops-stack")}>
          <span>{item.businessDate}</span>
          {item.dateState === "suspect_year" ? (
            <Tag tone="warning" title="工作台里写成了 2024 年">
              日期存疑
            </Tag>
          ) : null}
        </div>
      </td>
      <td>{f.platformLabel[item.platform]}</td>
      <td>
        <button type="button" onClick={() => openDetail(item)} aria-label={`查看发布记录：${item.title || '无标题'}`}
          className={c('ops-object', 'ops-clamp', ...(item.title ? [] : ['ops-secondary']))}
          title={item.title || (item.body ? '没有标题，显示正文开头' : '标题和正文未迁入，可查看记录详情')}>
          {item.title || item.body || '标题未迁入'}
        </button>
      </td>
      <td>
        <div className={c("ops-stack")}>
          <Tag title="系统还没去主页核对过这篇帖子；自动发现上线后接入">
            未检查
          </Tag>
          <span className={c("ops-secondary")}>
            {item.views.state === "value"
              ? "阅读量已填"
              : item.views.state === "pending"
                ? "阅读量待补"
                : "抖音选填"}
          </span>
        </div>
      </td>
      <td>
        <div className={c("ops-stack")}>
          <External href={item.url} title={item.url}>
            {item.url}
          </External>
          {item.duplicates.length ? (
            <Tag
              title={`已合并的重复登记日期：${item.duplicates.map((d) => d.businessDate).join("、")}`}
            >
              另有 {item.duplicates.length} 条重复登记
            </Tag>
          ) : null}
        </div>
      </td>
      <td>
        {item.cover.available ? (
          <button
            type="button"
            className={c("ops-cover")}
            aria-label="查看封面大图"
            onClick={() =>
              openCover(`/ops/covers/${encodeURIComponent(item.ref)}`)
            }
          >
            <img
              src={`/ops/covers/${encodeURIComponent(item.ref)}`}
              alt="封面"
              loading="lazy"
            />
          </button>
        ) : (
          <div
            className={c("ops-cover-missing")}
            title="这条记录在工作台没有封面，或封面还没拷来"
          >
            <span className={c("ops-cover-glyph")} aria-hidden="true">
              ▧
            </span>
            <span>封面未迁入</span>
          </div>
        )}
      </td>
      <TextCell value={item.body} />
      <td>
        {item.account ? (
          <div className={c("ops-stack")}>
            <span className={c("ops-ellipsis")} title={item.account.name}>
              {item.account.name}
            </span>
            {item.account.ledgerState !== "in_ledger" ? (
              <Tag title="这个账号已在工作台删除，旧状态未知">已停用</Tag>
            ) : null}
          </div>
        ) : (
          <Missing title="工作台里这条记录没有关联账号">账号未知</Missing>
        )}
      </td>
      <TextCell value={item.publisher} />
      <DisabledActions />
      <td className={c("ops-number")}>
        <div className={c("ops-stack")} style={{ alignItems: "flex-end" }}>
          {item.views.state === "value" ? (
            <>
              <span title={f.count(item.views.value).title || undefined}>
                {f.count(item.views.value).text}
              </span>
              {item.views.hit ? (
                <Tag tone="success" title="小红书阅读量不少于 5000">
                  爆款
                </Tag>
              ) : null}
            </>
          ) : item.views.state === "pending" ? (
            <>
              <Tag tone="warning" title="小红书阅读量还没人填，不是 0">
                待补
              </Tag>
              {item.views.overdue ? (
                <Tag tone="strong" title="发布已满 7 天仍未填">
                  满 7 天
                </Tag>
              ) : null}
            </>
          ) : (
            <Missing title="抖音播放量选填，不判爆款" />
          )}
        </div>
      </td>
      <NumberCell value={item.likes} />
      <NumberCell value={item.collects} />
      <NumberCell value={item.comments} />
      <td className={c("ops-number")}>
        {item.coverClickRate === null ? (
          <Missing />
        ) : (
          <span>{f.percent(item.coverClickRate, 2)}</span>
        )}
      </td>
      <td>
        <Missing title="选题池上线后指向对应选题" />
      </td>
    </tr>
  );
}
function AccountRow({ item, index }: { item: api.Account; index: number }) {
  return (
    <tr>
      <td className={c("ops-number")}>{index + 1}</td>
      <td>{f.platformLabel[item.platform]}</td>
      <td>
        {item.profileUrl ? (
          <External href={item.profileUrl} title={item.name}>
            {item.name}
          </External>
        ) : (
          <span className={c("ops-ellipsis")} title={item.name}>
            {item.name}
          </span>
        )}
      </td>
      <TextCell value={item.deviceCode} />
      <NumberCell value={item.followerCount} />
      <TextCell value={item.ipLocation} />
      <TextCell value={item.operator} />
      <td>
        {item.status ? (
          <span>{f.accountStatus[item.status] || item.status}</span>
        ) : (
          <Missing />
        )}
      </td>
      <td>
        <Tag title="改为运营手动填写，录入功能上线后开放">未填</Tag>
      </td>
      <td>
        {item.isRetained === null ? (
          <Missing />
        ) : (
          <span>{item.isRetained ? "是" : "否"}</span>
        )}
      </td>
      <TextCell value={item.notes} />
      <DisabledActions />
    </tr>
  );
}
const execution = {
  not_published: ["未完成", "warning", "当天没有这个运营人的发布登记"],
  praise: ["可表扬", "success", "当天有爆款或有加微"],
  pending_views: [
    "待补阅读量",
    "warning",
    "当天发的小红书帖子满 7 天还没填阅读量",
  ],
  done: ["已完成", "", "当天有发布登记，没有待处理项"],
};
function DailyRow({ item }: { item: api.Daily }) {
  const [label, tone, title] = execution[item.execution];
  return (
    <tr>
      <td>{item.businessDate}</td>
      <td>{item.weekday}</td>
      <TextCell value={item.operator} />
      <td>
        <Tag tone={tone} title={title}>
          {label}
        </Tag>
      </td>
      <NumberCell value={item.published} />
      <NumberCell value={item.linksOpened} />
      <NumberCell value={item.followersGained} />
      <td className={c("ops-number")}>
        {item.addRate === null ? (
          <Missing title="打开为 0 时没有添加率" />
        ) : (
          <span>{f.percent(item.addRate * 100)}</span>
        )}
      </td>
      <NumberCell value={item.hits} />
      <NumberCell value={item.monthFollowersGained} />
      <NumberCell value={item.monthPublished} />
      <td>
        <ActionButton type="button" disabled title={readonly}>
          编辑
        </ActionButton>
      </td>
    </tr>
  );
}
function DateDialog({
  from,
  to,
  close,
  save,
}: {
  from: string;
  to: string;
  close: () => void;
  save: (from: string, to: string) => void;
}) {
  const [error, setError] = useState('');
  const [draftFrom, setDraftFrom] = useState(from);
  const [draftTo, setDraftTo] = useState(to);
  const guard = useOpsLeave({ dirty: draftFrom !== from || draftTo !== to, onClose: close });
  return (
    <><Dialog title="ops-date-title" onClose={guard.requestClose}>
      <form
        onInvalid={event => { event.preventDefault(); const input = event.target; if (input instanceof HTMLInputElement) setError(input.validity.valueMissing ? "请选择开始和结束日期" : input.validity.badInput ? "请输入完整有效的日期" : "开始日期不能晚于结束日期"); }}
        onSubmit={(event) => {
          event.preventDefault();
          const form = new FormData(event.currentTarget);
          const start = String(form.get('from') || ''), end = String(form.get('to') || '');
          if (start && end && start > end) { setError('开始日期不能晚于结束日期'); return; }
          save(start, end);
        }}
      >
        <header>
          <h2 id="ops-date-title">日期范围</h2>
          <small>按北京时间的发布日，含首尾两天</small>
        </header>
        <div className={c("ops-dialog-body")}>
          <label>
            开始日期
            <input type="date" name="from" value={draftFrom} onChange={event => { setDraftFrom(event.target.value); setError(''); }} max={draftTo || undefined} />
          </label>
          <label>
            结束日期
            <input type="date" name="to" value={draftTo} onChange={event => { setDraftTo(event.target.value); setError(''); }} min={draftFrom || undefined} />
          </label>
        </div>
        {error ? <p role="alert">{error}</p> : null}
        <footer>
          <ActionButton type="button" onClick={() => save("", "")}>
            清空日期
          </ActionButton>
          <ActionButton type="button" onClick={guard.requestClose}>
            取消
          </ActionButton>
          <ActionButton type="submit" variant="primary">
            确认
          </ActionButton>
        </footer>
      </form>
    </Dialog>{guard.confirmation}</>
  );
}
