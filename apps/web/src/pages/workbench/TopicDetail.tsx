import { ActionButton } from "../../shared/ActionButton";
import { useState } from "react";
import type { WorkDocument } from "./workbenchModel";
import styles from "./WorkbenchPage.module.css";

export function TopicDetail({
  document,
  accounts,
  accountError,
  accountsState,
  onRetryAccounts,
  onChange,
  onCompose,
  onRead,
  onWrite,
}: {
  document: WorkDocument;
  accounts: { ref: string; name: string; platform: string }[];
  accountError?: string;
  accountsState: "pending" | "error" | "success";
  onRetryAccounts: () => void;
  onChange: (document: WorkDocument) => void;
  onCompose: (value: boolean) => void;
  onRead: () => void;
  onWrite: () => void;
}) {
  const [editing, setEditing] = useState(false);
  return (
    <section className={styles.proposal} aria-label="完整选题方案">
      <div className={styles.proposalMain}>
        <div className={styles.proposalMeta}>
          <small>选题方案</small>
          <ActionButton variant="quiet" onClick={() => setEditing(!editing)}>
            {editing ? "收起编辑" : "编辑方案"}
          </ActionButton>
        </div>
        {editing ? (
          <label>
            选题
            <input
              aria-label="选题标题"
              value={document.title}
              onCompositionStart={() => onCompose(true)}
              onCompositionEnd={() => onCompose(false)}
              onChange={(e) => onChange({ ...document, title: e.target.value })}
            />
          </label>
        ) : (
          <h1>{document.title || "未命名选题"}</h1>
        )}
        {editing ? (
          <label>
            切入角度
            <textarea
              aria-label="切入角度"
              value={document.angle}
              onCompositionStart={() => onCompose(true)}
              onCompositionEnd={() => onCompose(false)}
              onChange={(e) => onChange({ ...document, angle: e.target.value })}
            />
          </label>
        ) : (
          <p className={styles.proposalLead}>
            {document.angle || "切入角度待确定"}
          </p>
        )}
        <div className={styles.row}>
          <ActionButton variant="primary" onClick={onRead}>
            研读材料 →
          </ActionButton>
          <ActionButton variant="outline" onClick={onWrite}>
            直接创作
          </ActionButton>
        </div>
        <section>
          <h2>图文怎么展开</h2>
          {editing ? (
            <textarea
              aria-label="创作提纲"
              value={document.outline.join("\n")}
              onCompositionStart={() => onCompose(true)}
              onCompositionEnd={() => onCompose(false)}
              onChange={(e) =>
                onChange({ ...document, outline: e.target.value.split("\n") })
              }
            />
          ) : document.outline.some(Boolean) ? (
            <ol className={styles.proposalOutline}>
              {document.outline.filter(Boolean).map((text, index) => (
                <li key={index}>{text}</li>
              ))}
            </ol>
          ) : (
            <p className={styles.muted}>提纲尚未形成</p>
          )}
        </section>
      </div>
      <aside className={styles.proposalAside}>
        <section>
          <h2>选题依据</h2>
          {document.excerpts.length ? (
            document.excerpts
              .slice(0, 3)
              .map((excerpt) => (
                <blockquote key={excerpt.id}>
                  {excerpt.selectedText ?? "来源已受限或不可用"}
                </blockquote>
              ))
          ) : (
            <p>尚未摘录依据。进入研读查看本篇准备的相关材料。</p>
          )}
        </section>
        <section>
          <h2>可用材料</h2>
          <button className={styles.materialLink} onClick={onRead}>
            本篇材料与真实原声 <span>→</span>
          </button>
          <small>
            {document.materials.length} 份本篇材料 · {document.excerpts.length}{" "}
            段摘录 · {document.canvas.length} 张画布卡
          </small>
        </section>
        <section>
          <h2>待补与核对</h2>
          <p>
            {!document.angle.trim()
              ? "明确切入角度"
              : !document.outline.some(Boolean)
                ? "补齐创作提纲"
                : !document.excerpts.length
                  ? "核对相关材料并选定依据"
                  : "核对来源范围与自己的表达"}
          </p>
        </section>
        <section>
          <label>
            所属账号
            <select
              aria-label="所属账号"
              disabled={accountsState !== "success"}
              value={document.accountRef ?? ""}
              onChange={(e) =>
                onChange({ ...document, accountRef: e.target.value || null })
              }
            >
              <option value="">暂不绑定，导出前选定</option>
              {accounts.map((a) => (
                <option key={a.ref} value={a.ref}>
                  {a.name} ·{" "}
                  {(
                    {
                      xhs: "小红书",
                      douyin: "抖音",
                      wechat_channels: "视频号",
                    } as Record<string, string>
                  )[a.platform] ?? "平台名称待核对"}
                </option>
              ))}
            </select>
          </label>
          {accountError ? (
            <p role="alert" className={styles.error}>
              {accountError}
            </p>
          ) : null}
          {accountsState === "pending" ? (
            <small>正在读取所属账号…</small>
          ) : accountsState === "error" ? (
            <ActionButton variant="outline" onClick={onRetryAccounts}>
              重新读取账号
            </ActionButton>
          ) : null}
          {accountsState === "success" && !accounts.length ? (
            <small>这个领域还没有运营账号，导出前需登记。</small>
          ) : null}
        </section>
      </aside>
    </section>
  );
}
