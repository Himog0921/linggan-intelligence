# 本机常驻服务部署手册

> 状态: 权威当前
> 最后核对: 2026-09-03
> 适用范围: 本机三个 launchd 常驻服务（API / 巡检 worker / 媒体 worker）的运行来源、更新方式与故障处置
> 事实来源: Mog 于 2026-09-03 的明确要求「本地以 `/Users/moglenny/proma/linggan-intelligence` 为准，跟远端同步，不要到处复制」、当前 launchd 配置与实际运行验证
> 冲突时以谁为准: 实际运行输出与 launchd 当前加载的配置；本手册不授予平台访问或迁移执行权限

## 1. 唯一的两个目录

```
/Users/moglenny/proma/linggan-intelligence          ← 开发。就是本地 main，与 origin/main 同步
~/Library/Application Support/Linggan Intelligence/
    runtime-main/                                   ← 运行。detached，跟随 origin/main
```

**不再有第三个副本。** 2026-09-03 之前的做法是每次部署新建一个按 commit 命名的冻结快照
（`runtime-source/linggan-intelligence-origin-main-<sha>`），并把启动脚本里的路径改过去。
它累积到 28 份共 21GB，而且三个服务各自指向自己那次部署的快照——当天实测 API 在 `ccf7cca`、
两个 worker 还停在 `7a792c9`，同一台机器上跑着两个版本的代码。该模式已整体废弃。

开发目录之所以与运行目录分开，只有一个理由：**开发时的未提交改动不应该影响常驻服务**。
运行目录永远不接受手工编辑，每次启动都会被 `git reset --hard` 覆盖。

## 2. 更新方式

**重启服务即更新。** 没有第二个步骤。

```bash
launchctl kickstart -k gui/$(id -u)/com.linggan-intelligence.local-runtime
```

启动时 `runtime-launchers/sync-runtime-main.sh` 依次做：

1. 取同步锁（三个服务同时启动时串行化）；
2. `git fetch origin main` 并 `git reset --hard origin/main`；
3. 检查迁移台账，有未应用的迁移就**拒绝启动**（见下）；
4. 在锁内构建三个二进制；
5. 释放锁，各服务 `exec` 自己的二进制。

拿不到网络时不会让服务起不来：fetch 失败会记录一行并继续用当前 revision。

## 3. 迁移仍然不由服务执行

这条约束从旧模式沿用，理由没变：**一个开机自启的服务不该顺手改数据库结构**。

但代码现在会自动跟到 main，所以同步脚本必须主动检查——否则新代码撞上没跑的迁移，
报出来的是一堆 SQL 错误，而不是一句「该迁移了」。检查到有未应用迁移时服务拒绝启动，
日志里会列出是哪几个，处置方式固定：

```bash
cd /Users/moglenny/proma/linggan-intelligence
./scripts/local-runtime.sh migrate
```

**已知边界**：开机时 API 可能比 Docker PostgreSQL 先起来。此时读不到台账，脚本会记录
「读不到迁移台账」并**跳过检查继续启动**——硬拦会让服务在数据库没起来时永远起不来。
因此开机后第一次启动不保证迁移检查生效；手工重启一次即可获得完整检查。

## 4. 三个服务

| 服务 | launchd label | 启动脚本 | 二进制 |
|---|---|---|---|
| API（3000 端口） | `com.linggan-intelligence.local-runtime` | `run-linggan-api-worktree.sh` | `linggan-api` |
| 巡检调度 | `com.linggan-intelligence.patrol-worker` | `run-linggan-worker-worktree.sh` | `linggan-worker` |
| 媒体处理 | `com.linggan-intelligence.media-worker` | `run-linggan-media-worker.sh` | `linggan-media-worker` |

三者共用同一份 `sync-runtime-main.sh`，因此**永远跑同一个 revision**。

日志在 `runtime-logs/{api,worker,media-worker}.{out,err}.log`。

## 5. 处置

### 服务起不来，且日志里一行都没有

先看 launchd 自己的报错，不要看服务日志——服务还没跑到写日志就死了：

```bash
launchctl print gui/$(id -u)/com.linggan-intelligence.local-runtime | grep -iE 'working directory|last exit'
```

`last exit code = 78: EX_CONFIG` 且 `working directory` 指向一个不存在的目录，就是这个原因。
2026-09-03 清理快照时真实发生过：launchd 内存里加载的 job 仍带着指向已删快照的
`WorkingDirectory`，于是它在执行脚本前就失败。**服务日志里那时显示的报错是几天前的旧内容**，
按它排查会跑偏。

处置是重新加载 plist：

```bash
launchctl bootout gui/$(id -u)/com.linggan-intelligence.local-runtime
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.linggan-intelligence.local-runtime.plist
```

### 不要用 `pkill` 停服务

`pkill -f linggan-api` 会同时打到开发用的临时实例和常驻服务。按 label 操作：
`launchctl kickstart -k`（重启）或 `launchctl bootout`（停止）。

### 同步锁卡住

`runtime-main/.sync.lock` 是目录锁。超过 5 分钟的锁会被下次启动自动清理；要手工清就
`rmdir` 它。

## 6. 本手册不证明什么

- 不证明部署机（Mac mini）或任何远端环境采用同一模型——这里只描述本机；
- 不授权服务自行执行迁移、访问平台或修改数据库；
- 不保证开机首次启动时迁移检查一定生效（见第 3 节的已知边界）。
