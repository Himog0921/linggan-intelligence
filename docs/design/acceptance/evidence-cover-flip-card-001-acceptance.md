# ACC-EVIDENCE-COVER-FLIP-CARD-001 · 证据库封面翻转卡验收

> 状态: 一次性报告
> 交付状态: 交付分支已验证，未合并、未部署
> 最后核对: 2026-09-15
> 适用范围: `EVIDENCE-COVER-FLIP-CARD-001` 的源码、浏览器预览与未切换边界
> 事实来源: `codex/corpus-cover-flip-cards`、自动检查输出、受控本机只读预览
> 冲突时以谁为准: 真实分支、测试输出、实际运行服务和 Mog 前端验收

## 结论

封面排版的既有作品卡已在交付分支拆为可翻转主视觉和固定元数据板。正反面使用同一 3:4 Stage；标题最多两行且不会改变 Stage 起点或卡片高度。卡片其余区域继续复用原有行选择与 Inspector 链路。

## 已验证

| 层级 | 证据 | 结果 |
|---|---|---|
| 源码范围 | 仅 `evidence_library.js`、`evidence_library.css` 与局部文本合同测试改动 | 未改 API、数据模型、媒体资格、completeness、观察或采集路径 |
| 自动合同 | `cargo test -p linggan-api evidence_cover_layout_keeps_a_stationary_data_plate_and_one_shared_flip_stage` | 1 passed；锁定视觉/元数据分离、共享 Stage、同源封面、无 hover 点击阻断与 tooltip |
| 既有 Evidence 约束 | `cargo test -p linggan-api evidence_runtime` | 9 passed；受控媒体、3:4、未知/部分/受限状态、键盘/移动端与唯一读取投影均仍通过 |
| 静态检查 | `node --check apps/api/src/local_web/evidence_library.js`、`git diff --check`、`./scripts/verify-ui-design-handbook.sh` | 通过 |
| 桌面浏览器 | 1280px 受控预览，真实本机 Evidence 列表 50 条 | 一行与两行标题的 Stage 顶部对齐；键盘翻/回翻时视觉尺寸不变；背面本地真实 3:4 封面填满 Stage；信息板不翻；点击信息板仍更新 Inspector |

## 交互与数据核对

- 有 hover 的设备由主视觉 `rotateY(180deg)` 翻面；过渡使用现有 LIDS 时长组合（400ms）和既有 UI easing，不引入弹性效果。
- 无 hover 的设备仅点击主视觉切换前/后面，并对该次事件 `stopPropagation`；Enter/Space 同样可切换。卡片其他区域仍走既有详情选择。
- 正面只选择 4 个静态 SVG 模板（视频 wave、多图 stack、讨论 cluster、图文 frame）；没有 AI 生成、渐变、玻璃或新的媒体来源。
- 背面继续经过原有 `sameOriginPath` gate，只读取既有受控本机 `cover.localAssetUrl`。状态提示只汇总现有 material segment 事实；未知不伪装为 0。

## 未验证或未执行

- `:3000` 当前运行服务没有重启、替换或指向本分支；该地址仍不是本交付的运行回执。
- 本次浏览器预览由临时 `127.0.0.1:3108` 只读代理提供分支 JS/CSS，并转发至已有本机读取接口；没有执行 API、数据库、采集、平台或媒体写入。它不是部署。
- 移动设备的真实触摸浏览器、Mog 前端验收、合并、推送和部署均为 **NOT VERIFIED**；代码路径和文本合同已覆盖无 hover 点击语义。
- `cargo fmt --all -- --check` 仍因本分支基线中与本包无关的 `station_view.rs`、评论研究文件等格式差异失败；本包唯一 Rust 改动 `apps/api/src/local_web/tests.rs` 已单独格式化，且聚焦测试通过。

## 交付位置

- 分支：`codex/corpus-cover-flip-cards`
- Worktree：`/Users/moglenny/proma/linggan-intelligence/.worktrees/corpus-cover-flip-cards`
- 关联变更清单：[evidence-cover-flip-card-001-ui-change-manifest.md](../changes/evidence-cover-flip-card-001-ui-change-manifest.md)
