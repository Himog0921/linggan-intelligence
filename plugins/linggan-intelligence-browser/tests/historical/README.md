# 历史测试材料

> 状态: 历史保留（不随包发布）
> 最后核对: 2026-09-21
> 事实来源: 本目录的 source、`webpack.config.cjs` 的 content 入口图、`scripts/verify-linggan-isolation.mjs`
> 冲突时以谁为准: 当前可构建 source 与实际打包产物；本目录只说明这块代码为什么还在这里

## 这里放什么

放**已经不在包里跑的**模块，但仍有测试在读它们。默认保留、降级，不删除——删除会让「这块代码
曾经怎么做的」只剩下口头记忆；继续留在 `src/content/` 里，则会让每个读源码的人以为它是现役
入口。`MIGRATION-MAP.md` 里那一行「旧 source 仅作历史保留」说的就是这件事；本目录是它的落点。

## `douyinBatchMessageHandlers.js`

抖音批量视频/评论消息处理的旧实现。**它曾经是内容侧的热路径**（由 `src/content/index.js`
引入），现在不是了。

三条证据说明它不在包里跑：

1. `src/` 下**零**引用；
2. 它不在 `webpack.config.cjs` 的 content 入口图里，也就不进任何构建产物；
3. `scripts/verify-linggan-isolation.mjs` 断言 `src/content/index.js` 的**文本里不得出现**
   `douyinBatchMessageHandlers`——这条断言早于本次搬迁，本目录只是让文件位置与它一致。
4. **构建产物里也搜不到它**：`npm run build` 之后在新的 `dist/` 上搜这个名字，命中 0 个文件
   （2026-09-21 实测）。前两条说的是「入口图里没有」，这一条说的是「产物里确实没有」——
   入口图是推理，产物是事实。

谁还在读它：`tests/` 下的四个用例（`douyin-batch-remote-startup`、`douyin-batch-ui-routing`、
`douyin-batch-summary`、`manual-execution-lock-release`）与 `tests/task-state-constants.test.mjs`
的两条路径条目。它们读的是这个模块**自己的行为**（消息路由、任务状态词表、执行锁释放），
不是「它在包里的行为」。

## 改它的规矩

- **改行为前先问它还算不算历史材料**：它现在只服务测试。要让插件重新用上批量处理，那是新
  功能，要走派定 Issue、专属 branch/worktree 与独立复审，不是把 import 挪回 `src/` 就算接通。
- **保持词表一致**：它引用的 `TASK_STATE`、`MSG` 等常量仍来自 `src/shared/`。改了那些常量而
  这里不跟着改，`task-state-constants` 会红——这是有意的，历史材料也要如实反映它当初的写法。
- **不要往这里加新模块**：这里是「已停用但仍被测试读取」的位置。新增的能力属于 `src/`。
