import { humanMoment } from '../../shared/formatting';
import { ActionButton } from "../../shared/ActionButton";
import { FindField } from '../../shared/FindField';
import { useState, type ReactNode } from "react";
import { useQueries } from "@tanstack/react-query";
import * as api from "./workbenchApi";
import type { IdeaBoxSummary, WorkSummary } from "./workbenchModel";
import { topicRows } from "./topicRows";
import styles from "./WorkbenchPage.module.css";

export function TopicLibrary({
  section,
  works, ideas = [], ideasStatus = 'success', onRetryIdeas,
  onOpen, action, loading = false, failed = false,
}: {
  section: "today" | "works";
  works: WorkSummary[];
  ideas?: IdeaBoxSummary[];
  ideasStatus?: 'pending' | 'error' | 'success'; onRetryIdeas?: () => void;
  onOpen: (id: string) => void;
  action?: ReactNode; loading?: boolean; failed?: boolean;
}) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState("all");
  const rowsToRead = section === 'today' ? topicRows(works, ideas) : works;
  const detail = useQueries({
    queries: rowsToRead.map((work) => ({
      queryKey: ["creation-proposal", work.workId, work.revisionId],
      enabled: section !== 'today' || !work.ideaRef || ideas.some(idea => idea.ideaRef === work.ideaRef),
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        api.readWork(work.workId, signal),
      gcTime: 0,
    })),
  });
  const entries = rowsToRead.map((work, index) => {
    const unavailable = section === 'today' && !!work.ideaRef && !ideas.some(idea => idea.ideaRef === work.ideaRef);
    return {
      work,
      envelope: unavailable ? undefined : detail[index]?.data,
      loading: !unavailable && detail[index]?.isPending && detail[index]?.isFetching,
      failed: !unavailable && detail[index]?.isError,
      legacy: !unavailable && api.isLegacyReferenceUnavailable(detail[index]?.error),
      retry: () => detail[index]?.refetch(),
    };
  });
  const unresolved = entries.filter((entry) => !entry.envelope);
  const rows = entries.filter(({ work, envelope }) => {
    const d = envelope?.revision.document;
    return (
      `${work.title} ${d?.angle ?? ""}`.includes(query) &&
      (filter === "all" ||
        (filter === "prepared"
          ? !!d?.materials.length
          : !!d && !d.paragraphs.some((p) => p.text.trim())))
    );
  });
  return (
    <>
      <div className={styles.libraryToolbar}>
        <FindField value={query} onCommit={setQuery} label="搜索选题或角度" placeholder="选题名称、切入角度" trigger="debounce_250" trim={false} shortcut />
        {action}
      </div>
      <div className={styles.libraryFilters}>
        <div className={styles.steps} role="group" aria-label="选题筛选">
          {[
            { id: "all", label: "全部" },
            { id: "prepared", label: "已有材料" },
            { id: "unwritten", label: "待创作" },
          ].map((item) => (
            <button
              key={item.id}
              aria-pressed={filter === item.id}
              onClick={() => setFilter(item.id)}
            >
              {item.label}
            </button>
          ))}
        </div>
      </div>
      {section === 'today' && works.some(work => work.ideaRef) && ideasStatus !== 'success' ? <div className={styles.libraryReadStatus} role="status">
        {ideasStatus === 'pending' ? '正在读取选题框…' : <>选题框读取失败，关联选题暂不可打开。 <ActionButton variant="outline" onClick={onRetryIdeas}>重新读取选题框</ActionButton></>}
      </div> : null}
      {unresolved.length ? (
        <div className={styles.libraryReadStatus} role="status">
          {unresolved.some((entry) => entry.loading) ? (
            <p>正在读取方案详情，筛选结果尚不完整。</p>
          ) : null}
          {unresolved
            .filter((entry) => entry.failed)
            .map((entry) => (
              <p key={entry.work.workId}>
                {entry.work.title}：{entry.legacy
                  ? "历史引用缺少当前所需的来源定位，原稿已保留，暂不能安全打开。"
                  : <>
                    详情读取失败{" "}
                    <ActionButton
                      variant="outline"
                      onClick={() => void entry.retry()}
                    >
                      重试读取方案
                    </ActionButton>
                  </>}
              </p>
            ))}
        </div>
      ) : null}
      {section === "today" ? (
        <div className={styles.topicWall}>
          {rows.map(({ work, envelope, loading, failed, legacy }, index) => {
            const d = envelope?.revision.document;
            const ideaUnavailable = !!work.ideaRef && !ideas.some(idea => idea.ideaRef === work.ideaRef);
            return (
              <button
                className={styles.topicCard}
                key={work.workId}
                disabled={ideaUnavailable}
                onClick={() => onOpen(work.workId)}
              >
                <div className={styles.topicArt}>
                  <small>{String(index + 1).padStart(2, "0")}</small>
                  <strong>{work.title || "未命名选题"}</strong>
                </div>
                <div className={styles.topicCopy}>
                  <h2>
                    {ideaUnavailable ? "选题信息暂不可用" : d?.angle || (loading ? "读取方案中…" : "切入角度待确定")}
                  </h2>
                  <p>
                    {ideaUnavailable ? "关联选题尚未读取" : d?.outline.filter(Boolean).slice(0, 2).join(" / ") ||
                      (legacy ? "历史引用无法安全读取，原稿已保留" : failed ? "方案读取失败，打开后重试" : "提纲待整理")}
                  </p>
                  <footer>
                    <span>
                      {d
                        ? `${d.materials.length} 份本篇材料 · ${d.materialPreparation?.state === "insufficient" ? "待补" : d.materialPreparation ? "已准备" : "待核对"}`
                        : "材料状态待读取"}
                    </span>
                    <span>查看方案 ↗</span>
                  </footer>
                </div>
              </button>
            );
          })}
        </div>
      ) : (
        <div className={styles.draftList}>
          {rows.map(({ work, envelope, loading, failed, legacy }) => {
            const d = envelope?.revision.document;
            const words = d
              ? Array.from(d.paragraphs.map((p) => p.text).join("")).length
              : null;
            return (
              <button key={work.workId} onClick={() => onOpen(work.workId)}>
                <div>
                  <strong>{work.title || "未命名选题"}</strong>
                  <p>
                    {d
                      ? d.paragraphs.find((p) => p.text.trim())?.text ||
                        "尚未开始正文"
                      : legacy
                        ? "历史引用无法安全读取，原稿已保留"
                        : failed
                        ? "稿件详情读取失败"
                        : "正在读取稿件详情…"}
                  </p>
                </div>
                <small>
                  {d
                    ? `${words} 字 · ${d.pages.filter((p) => p.kind !== "cover").length} 张内页`
                    : failed
                      ? legacy
                        ? "历史引用缺少来源定位"
                        : "读取失败，进度未知"
                      : loading
                        ? "正在读取进度…"
                        : "进度未知"}
                  <br />
                  {humanMoment(work.savedAt)}
                </small>
                <span>继续创作 →</span>
              </button>
            );
          })}
        </div>
      )}
      {loading ? <p role="status">正在读取已保存作品…</p> : null}
      {!loading && !failed && !rows.length && !unresolved.length ? (
        <div className={styles.empty}>
          {works.length
            ? "没有匹配的方案，试试其他词或清除筛选。"
            : section === "today"
              ? "还没有已保存的真实方案。可从上方手动建立一篇。"
              : "还没有稿件。建立选题后，这里保留你的正文和图页进度。"}
        </div>
      ) : null}
    </>
  );
}
