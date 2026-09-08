# ACC-TARGET-INSPECTOR-PERFORMANCE-001 · 观察目标检查器验收

> 状态: 一次性报告
> 最后核对: 2026-09-08
> 适用范围: Issue #158 / branch `codex/target-inspector-performance-001`
> 事实来源: 当前交付分支、专属测试、disposable PostgreSQL 与隔离浏览器预览
> 冲突时以谁为准: 真实代码/数据库/浏览器结果、用户最新确认与 Issue Claim

## 验收对象

- 页面：`/collection/targets`、creator target Drawer、作品列表/表现、巡查。
- 视口：1440 CSS px 桌面全屏；窄屏不在本包支持范围，但不得破坏 Drawer 的既有边界。
- 数据前提：只读隔离库；合成来源必须标明不是 Evidence；不接触共享库或外部平台。

## 场景矩阵

| 场景 | 预期 | 结果 |
|---|---|---|
| 六目标选择 | 独立勾选、顶部数量、分组全选、勾选不打开 Drawer | BROWSER + SOURCE VERIFIED；浏览器勾选 2 项后按钮显示 `批量编辑 2`，弹窗显示 `已选择 2 个目标`；专属 UI 合同 8/8 |
| 正常/排队/执行 | queued 与 running 分开；自动处理中无人工按钮 | SOURCE + POSTGRES VERIFIED |
| 部分/阻塞 | 已取得事实保留，缺口与唯一救援动作可见 | SOURCE + POSTGRES VERIFIED |
| Known zero / Unknown | 零可见；未知不入图、不伪造零 | SOURCE + POSTGRES VERIFIED |
| 作品列表/表现 | URL 可恢复；列表查证、表现看分布 | BROWSER + SOURCE VERIFIED；`wview=performance` 可恢复，既有 lifecycle 回归 16/16 |
| 无分类 | 明示尚未建立内容分类 | BROWSER + SOURCE VERIFIED；空样本没有生成主题结论 |
| 1440/a11y | 主体列与操作同时可见；控件、focus、reduced motion、图表替代成立 | VERIFIED；1440×900 三张实拍；AX table/link/button 语义、关闭后目标 focus hash 与 0 browser error 已核对 |

## 分层结论

| 完成层 | 状态 | 证据/限制 |
|---|---|---|
| 设计规格一致 | VERIFIED ON MAIN + LIVE LOCAL PAGE | 1440×900 列表、概览、作品表现实拍；发布后 Chrome 实页再次核对新三 Tab、页头动作和已删除旧文案 |
| 源码实现 | VERIFIED ON MAIN | PR #205 exact head `e067bfb` 已合并为 `f0ed68d` |
| 自动检查 | VERIFIED | focused、workspace、格式、Node、diff、项目治理与 UI handbook 均通过 |
| 隔离 PostgreSQL | TARGET INSPECTOR VERIFIED | PostgreSQL 16 的新 inspector proof 1/1 证明 queued/running、Known zero/Unknown 与只读无副作用；完整旧脚本随后在未修改的 `collection_control_postgres` 两项基线断言处失败，不把整条脚本写成全绿 |
| 本机 API 发布 | VERIFIED | PR #205 合并为 `f0ed68d`；PID `54442` 实际来自 `runtime-main@f0ed68d`，health READY，Targets 200，Chrome 实页 6 目标与 0 console issue |
| Mog / 业务验收 | NOT VERIFIED | 需后续前端验收 |

## 不得推断

源码或截图通过不证明 shared runtime 已替换、采集成功、作品完整、分类已建立、跨账号价值成立或业务验收完成。

本次后续授权只刷新了本机 loopback API；没有新 migration，也没有重启 worker、重载插件、访问平台或触发真实采集。这里的 runtime 证明不扩张为那些层面的完成声明。

## 完整 PostgreSQL 脚本保留失败

`./scripts/test-local-001-discovery-postgres.sh` 已实际运行。新增 `target_inspector_postgres` 1/1 通过，临时 database/container/volume 已清理；脚本后续在本分支未修改的 `collection_control_postgres` 留下两项失败：一项仍把迁移边界写死在 `0042`，而当前证明集合早已继续增长；另一项为按时点变化的 station daily-limit 预期与当前 available 结果不一致。它们不改变本包 inspector 专项证明，但阻止我们宣称完整 PostgreSQL harness 全绿。
