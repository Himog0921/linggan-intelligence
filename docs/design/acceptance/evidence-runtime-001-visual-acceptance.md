# ACC-EVIDENCE-RUNTIME-001 · 多材料证据库运行页验收

> 状态: 一次性报告
> 最后核对: 2026-08-29
> 适用范围: Issue #90 / EVIDENCE-RUNTIME-001 在 `/corpus/evidence` 的运行页面、Material Projection 读取边界、桌面与 375px 表达
> 事实来源: 当前分支 Rust/HTML/CSS/JS、聚焦自动检查、`http://127.0.0.1:3090` 本机浏览器 DOM 与截图
> 冲突时以谁为准: 真实运行/代码/API 合同、用户最新确认；本报告不替代部署、真实垂直证明或 Mog 业务验收

## 1. 结论

Issue #90 获准范围通过：运行页不再 server-render legacy discovery cards，默认只消费 `/api/local/evidence-library` Material Projection；页面具有作品级九材料通道、按 `detailUrl` 更新的 Inspector、本机授权评论研究读取、受控媒体安全内联、有界 continuation、键盘 Tab/作品行合同与 375px 顺序流。

本次本机持久库没有 0015–0018 Material Projection schema，真实 API 返回 `503 material_projection_schema_unavailable`。页面正确显示“未读取任何作品材料”并明确无 legacy/远程 CDN fallback；因此本次浏览器证据证明运行页面和失败状态，不证明该库中的作品 lane/评论/媒体已经真实读出。没有为截图迁移或改写共享数据库。

## 2. 自动检查

| 检查 | 结果 | 证明 |
|---|---|---|
| `./scripts/verify-evidence-library-reference.sh` | PASS | PR #87 产品参考、状态与无远程依赖仍完整 |
| `./scripts/verify-ui-design-handbook.sh` | PASS | LIDS/UI 手册结构与索引成立 |
| `cargo fmt --all -- --check` | PASS | Rust 格式无差异 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS | workspace/all targets 无警告 |
| `cargo test -p linggan-api evidence_runtime_` | PASS，4/4 | 唯一 Material API、状态诚实、评论/媒体边界、continuation/键盘/响应式合同 |

聚焦测试首轮有一项测试文案断言过宽：它把“这不表示作品没有媒体”中的反例说明误判为页面声称“没有媒体”。只修正该断言为确认反例说明存在，随后同一聚焦组 4/4 通过；运行代码和合同未为测试结果改变。

## 3. 浏览器证据

| 视口 / 场景 | 实测结果 |
|---|---|
| 1440×900 桌面 | document `scrollWidth=clientWidth=1440`；三栏为 176px 状态视图、620px 结果、430px Inspector；4 个 Inspector Tab；无外部资源、无 legacy URL；诚实显示 schema unavailable |
| 375×812 窄屏 | document 与 main 均 `scrollWidth=clientWidth=375`；command/results/Inspector 均 375px；bench 为顺序 `block`；全部可见页面控件高度 ≥40px |
| 键盘 | 概览 Tab 按 ArrowRight 后“评论研究”成为 `aria-selected=true` 且获得焦点；Tab 使用 roving tabindex |
| 安全/数据 | 页面未生成作品行，未伪造 lane；失败文案明确没有读取任何材料、不会回退旧卡片/远程数据库/CDN |

375px 首次检查发现 `.ev-main` 的隐式 grid column 使用内容最小宽度，内部 `scrollWidth=806`。一次性修正为显式 `minmax(0,1fr)` 后，document/main/command/results/Inspector 全部收敛为 375px；随后只复核该缺陷与更新截图，没有重跑整套验收。

临时视觉资产（未登记生成物，不提交 Git）：

- `/var/folders/pr/kfmym_dd4z54mvfrknqdw0f40000gn/T/evidence-runtime-desktop-1440x900.png`
- `/var/folders/pr/kfmym_dd4z54mvfrknqdw0f40000gn/T/evidence-runtime-mobile-375x812-top.png`
- `/var/folders/pr/kfmym_dd4z54mvfrknqdw0f40000gn/T/evidence-runtime-mobile-375x812-inspector.png`

## 4. 状态与真实后果

- `UNKNOWN`、部分、风险停止、受限、已清理和读取失败均有独立呈现；数量未知不变成 0。
- 列表没有原始评论、平台用户标识或 `contentExternalId`；评论正文只由授权详情通道以 `textContent` 显示匿名上下文。
- 媒体内联同时要求受控同源 URL、`INLINE_SAFE` 与安全 image/video MIME；未知/SVG/HTML/attachment/受限/已清理不内联。
- 回执截断但后端没有通道 URL时，页面显示 `SOURCE_INCOMPLETE`，不猜测路由。
- 浏览器网络只访问 127.0.0.1 当前服务；没有写请求、平台访问、远程 CDN、插件动作或数据库变更。

## 5. 未证明

- 当前持久库的 Material Projection schema 已迁移、历史 Package 已回填或已有作品能在运行页显示九通道；
- 真实评论分页、媒体副本、Live Photo、OCR/ASR 派生与 provenance 下一页在该库中工作；
- 真实平台、账号、插件调度、采集稳定性、部署或长期运行；
- 辅助技术全量认证、性能/负载、Mog 最终视觉与业务验收。
