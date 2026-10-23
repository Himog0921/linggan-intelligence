# ACC-OCR-MANUAL-HUMAN-ADOPTION-454 · 验收记录

> 状态: 一次性报告
> 最后核对: 2026-10-23
> 适用范围: #454 手动云端 OCR 的人工采用候选
> 事实来源: 本包代码、合成测试和受控回执；结果以实际命令输出补记
> 冲突时以谁为准: 真实运行、数据合同、Mog 最新确认


本报告保留 #479 当时的证据，不代表重构后的 v2 已通过。当前实现与新验证见 [v2 实施包](../../plans/active/research-dossier-v2.md)。

## 验收对象

- 页面/组件/状态：Evidence Library 图组中的人工采用表单、已保存 OCR 结果、回执刷新。
- 关联规则：PAGE-EVIDENCE-001、OCR-MANUAL-HUMAN-ADOPTION-454、LIDS-INTERACTION-001。
- 环境：候选 worktree + 合成 loopback + owned disposable PostgreSQL；不读取共享 DSN 或真实图片。

## 场景矩阵

| 场景 | 用户任务 | 预期状态含义 | 真实后果/回执 | 结果 |
|---|---|---|---|---|
| 保存 cloud preview | 对照合成受控原图后采用完整正文 | 机器预览仍独立；人工版本仅确认后显示 | 新 immutable command/review，读回 `human_ocr_review` 与 dossier 片段 | VERIFIED（owned PG） |
| 无结果/不合格来源 | 看到无可采用入口 | 不等于没有 OCR 或图片 | 未采用 preview 保持不可作为材料；撤回/退休尚未覆盖 | 部分 VERIFIED |
| 网络未知 | 选择核对同一请求 | 不新建第二版 | 相同 command identity 重放原 receipt | VERIFIED（owned PG） |
| 版本冲突/撤回 | 重新读取而非覆盖 | 当前版本或来源已变化 | revision 冲突已拒绝；撤回尚未覆盖 | 部分 VERIFIED |

## 分层结论

| 完成层 | VERIFIED / NOT VERIFIED / N/A | 证据 | 仍有限制 |
|---|---|---|---|
| 设计规格一致 | VERIFIED（代码级） | 实现限制为 `adopted_body`，合同/计划/清单同步；cloud 读取复用 review overlay | 尚未由 Mog 视觉验收 |
| 前端/组件实现 | VERIFIED（隔离） | `ManualOcrAdoption` 覆盖资格、固定 `adopted_body`、receipt 后刷新、未知回执复用 command 与 409 文案；真实 React 浏览器完成提交/回读 | 不等同真实人员判断 |
| 自动检查 | VERIFIED（本包） | Node 24.13.0 下 `npm run verify`、LIDS、治理、生产 build、Rust/API 定向和 `git diff --check` | 远端 main-base CI 仍待临时集成 PR |
| 受控数据链路/回执 | VERIFIED（owned PG + browser） | `bash scripts/test-ocr-quality-audit-postgres.sh --browser`；未采用 cloud preview 无 OCR material fragment，receipt 后仅人工正文由 note-material/dossier/OCR run 读回；无共享 DSN、真实图片或真实 provider | 不等同共享库、生产或真实原图 |
| 部署 | NOT VERIFIED | 本包无部署授权 | 需独立授权 |
| Mog / 业务验收 | NOT VERIFIED | G4 保留给 Mog 实际原图核验 | 不得由测试代签 |

## 已执行命令与边界

- 2026-10-23：`bash scripts/test-ocr-quality-audit-postgres.sh --manual-adoption` 通过；脚本只启动并清理 owned disposable PostgreSQL、volume 与合成媒体资产。它没有配置模型、调用 provider、外发图片/文本或访问共享 DSN。该回归还构造了错误的跨作品 recognition item 并确认拒绝，且在采用前确认 cloud preview 的 `researchText`/`imageSubstantiveText` 均为 null。
- 2026-10-23：`cargo test -p linggan-intelligence ocr_manual_adoption --locked`、`cargo check -p linggan-api --locked` 和 `git diff --check` 通过；编译输出只有现存未使用 helper 警告。
- 2026-10-23：按 `apps/web/package-lock.json` 在 Node 24.13.0 执行 `npm ci --prefix apps/web --ignore-scripts --no-audit --no-fund`，不改 lockfile；随后 `npm --prefix apps/web run verify`、`npm --prefix apps/web run lids:verify` 及人工采用组件定向均通过。npm 对三个现有依赖给出偏好 Node 24.15+ 的 EBADENGINE warning，未改变安装结果或源码。
- 2026-10-23：`bash scripts/test-ocr-quality-audit-postgres.sh --browser` 通过。它以 disposable PostgreSQL、合成 2×2 图片、Axum 与已构建 React 运行；provider 仅为 loopback mock，无真实 provider、图片、共享 DSN 或运行时服务。脚本确认其自有容器、卷和临时资产已清理。
