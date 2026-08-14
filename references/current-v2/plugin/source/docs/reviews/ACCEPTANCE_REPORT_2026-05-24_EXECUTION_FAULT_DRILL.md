# 执行控制链路故障演练验收记录

日期：2026-05-24

范围：插件后台、内容页、任务租约、账号执行锁、增量同步队列。

## 结论

代码级故障演练通过。当前执行链路已经覆盖以下高风险场景：

- 后台被 Chrome 回收后，任务上下文可以从本地记录恢复。
- 页面关闭、刷新或内容脚本断连时，任务不会永久停在 running。
- 同一平台、同一账号的并发任务只允许一个执行。
- 工作台发出的暂停、继续、删除控制可以落到本地执行态。
- 插件创建的辅助执行页可以被记录、去重和清理。
- 抽取结果结构不合格时，任务会失败并输出健康告警，不再伪装成功。

## 本轮自动演练

执行命令：

```bash
node --test tests/workbench-task-poller.test.mjs tests/execution-account-lock.test.mjs tests/shared-messaging-send-to-tab.test.mjs tests/background-task-target-resolution.test.mjs tests/workbench-task-execution-cleanup.test.mjs tests/navigation-orchestrator.test.mjs tests/manual-execution-lock-release.test.mjs tests/workbench-control-sync.test.mjs
```

结果：74 条通过，0 条失败。

## 场景映射

| 场景 | 验收依据 | 结果 |
| --- | --- | --- |
| Service Worker 被回收后恢复任务上下文 | `task poller recovers persisted context after worker restart` | 通过 |
| 旧 attempt 不应错误恢复 | `task poller ignores persisted context from a stale attempt` | 通过 |
| 用户关闭任务页 / 内容脚本断连 | `task poller releases a leased task when the content script is unavailable` | 通过 |
| 页面刷新后重注入内容脚本 | `sendToTab reinjects content script and retries recoverable tab context errors` | 通过 |
| 关闭 tab 后不复用死页 | `selectReachableTaskTab skips dead tabs and keeps trying the next live xhs page` | 通过 |
| 同账号双任务互斥 | `execution account lock rejects another task on the same platform account` | 通过 |
| 同账号并发抢锁只成功一个 | `execution account lock serializes concurrent acquisition for the same platform account` | 通过 |
| 任务结束释放账号锁 | `task poller releases the account execution lock when a task finishes` | 通过 |
| 手动批量任务释放账号锁 | `xhs manual batch releases its execution lock after the task finishes` / `douyin manual batch releases its execution lock after the task finishes` | 通过 |
| 工作台暂停控制 | `task poller applies workbench pause control and emits applied plus paused events` | 通过 |
| 工作台删除控制映射为停止 | `task poller maps workbench delete control to local stop semantics` | 通过 |
| 辅助执行页清理 | `task execution cleanup covers task id, external id, and plugin run id` | 通过 |
| 后台导航不抢当前可见页 | `selectReachableTaskTab avoids hijacking the currently visible tab when a background candidate is available` | 通过 |

## 未混淆的边界

本报告证明的是可重复的执行链路故障演练，不把它伪装成登录态平台手工采集验收。真实账号登录态下的完整页面点击采集仍应在发布候选包安装后做一次冒烟复验，重点看：

- 小红书作者页批量采集刷新后继续。
- 抖音搜索页批量采集刷新后继续。
- 登录态风控弹层出现时，任务进入暂停或明确失败，而不是假成功。
