# ACC-OCR-MANUAL-HUMAN-ADOPTION-454 · 验收记录

> 状态: 一次性报告
> 最后核对: 2026-10-23
> 适用范围: #454 手动云端 OCR 的人工采用候选
> 事实来源: 本包代码、合成测试和受控回执；结果以实际命令输出补记
> 冲突时以谁为准: 真实运行、数据合同、Mog 最新确认

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
| 前端/组件实现 | NOT VERIFIED | 表单/API/刷新已实现；worktree 缺少 `vitest` 可执行文件，未能跑前端定向测试 | 待依赖齐全后的组件/浏览器验证 |
| 自动检查 | VERIFIED（Rust 定向） | `cargo test -p linggan-intelligence ocr_manual_adoption --locked`、`cargo check -p linggan-api --locked`、`git diff --check` | 全量 Rust/Web 套件未跑 |
| 受控数据链路/回执 | VERIFIED（owned PG） | `bash scripts/test-ocr-quality-audit-postgres.sh --manual-adoption`；确认无模型/图片外发、追加/幂等/冲突、sample/issue 均为 null、observation 未改写、dossier 读回 | 不等同共享库、生产或真实原图 |
| 部署 | NOT VERIFIED | 本包无部署授权 | 需独立授权 |
| Mog / 业务验收 | NOT VERIFIED | G4 保留给 Mog 实际原图核验 | 不得由测试代签 |

## 已执行命令与边界

- 2026-10-23：`bash scripts/test-ocr-quality-audit-postgres.sh --manual-adoption` 通过；脚本只启动并清理 owned disposable PostgreSQL、volume 与合成媒体资产。它没有配置模型、调用 provider、外发图片/文本或访问共享 DSN。该回归还构造了错误的跨作品 recognition item 并确认拒绝，且在采用前确认 cloud preview 的 `researchText`/`imageSubstantiveText` 均为 null。
- 2026-10-23：`cargo test -p linggan-intelligence ocr_manual_adoption --locked`、`cargo check -p linggan-api --locked` 和 `git diff --check` 通过；编译输出只有现存未使用 helper 警告。
- 2026-10-23：`npm --prefix apps/web test -- --run apps/web/src/pages/corpus/evidence/OcrRecognition.test.tsx` 未执行成功，因为该 worktree 没有 `vitest` 可执行文件；没有安装依赖或改动 lockfile。
