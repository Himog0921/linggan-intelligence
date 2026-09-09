# 本机常驻服务部署手册

> 状态: 权威当前
> 最后核对: 2026-09-08
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

## 2. 安装（重装机器或换机器）

部署脚本住在仓库 `scripts/runtime/`，不是散在 `~/Library` 里的手写副本。全新机器上：

```bash
git clone <repo> && cd linggan-intelligence
cp env.example .env && $EDITOR .env      # 填数据库口令
./scripts/dev-db.sh up                   # 起 PostgreSQL
./scripts/local-runtime.sh migrate       # 应用迁移
./scripts/runtime/install.sh             # 建运行目录、写 launchd、启动三个服务
```

`install.sh` 幂等：已装好时重跑只刷新 plist 并重启。它**不**跑迁移、不装依赖、不碰远端环境。

## 3. 更新方式

CI-AUTO-004 源码将版本切换改为先等待评论调用落账。以下是本包发布后的更新合同，不表示当前常驻进程已经使用这版协议。

```bash
./scripts/runtime/install.sh
```

`install.sh` 先请求巡检 worker 停止领取新模型任务，等待它将当前调用和用量落账，并核对与 PID 对应的退出回执。成功后才生成绑定起止 revision 的更新许可，再同步、构建并重新加载三个服务。未确认退出时停止更新，不把未知调用费用当作零。

启动时 `scripts/runtime/sync.sh` 依次做：

1. 取同步锁（三个服务同时启动时串行化）；
2. `git fetch origin main`；需要改变 revision 时核对并消费匹配的 worker 更新许可，之后才切换；
3. 检查迁移台账，有未应用的迁移就**拒绝启动**（见下）；
4. 在锁内构建三个二进制；
5. 释放锁，各服务 `exec` 自己的二进制。

拿不到网络时不会让服务起不来：fetch 失败会记录一行并继续用当前 revision。

## 4. 迁移仍然不由服务执行

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

## 5. 三个服务

| 服务 | launchd label | 二进制 |
|---|---|---|
| API（3000 端口） | `com.linggan-intelligence.local-runtime` | `linggan-api` |
| 巡检调度 | `com.linggan-intelligence.patrol-worker` | `linggan-worker` |
| 媒体处理 | `com.linggan-intelligence.media-worker` | `linggan-media-worker` |

三者由同一个 `scripts/runtime/launch.sh <binary>` 启动、共用同一份 `sync.sh`。更新后仍须分别核对三个 PID 和构建 revision；共用脚本不证明当前存活进程已经同步。

日志在 `runtime-logs/{api,worker,media-worker}.{out,err}.log`。

## 6. 处置

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

`pkill -f linggan-api` 会同时打到开发用的临时实例和常驻服务。版本更新使用上述受控 install 入口，不以 `kickstart -k` 绕过模型落账。开发用 `local-runtime.sh serve` 同样先等待自己启动的 worker，成功才结束其他子进程；失败保留回执并说明未完成停止。

### 同步锁卡住

`runtime-main/.sync.lock` 是目录锁。超过 5 分钟的锁会被下次启动自动清理；要手工清就
`rmdir` 它。

## 7. 本手册不证明什么

- 不证明部署机（Mac mini）或任何远端环境采用同一模型——这里只描述本机；
- 不授权服务自行执行迁移、访问平台或修改数据库；
- 不保证开机首次启动时迁移检查一定生效（见第 4 节的已知边界）。


## 8. CI-AUTO-004 评论自动研究的发布前条件

本段是未发布代码包的部署要求，不表示共享数据库或 3000 已更新。

- 追加迁移为 `0053`–`0060`，包含规则、自动政策、回放、语义原子、恢复、字段补齐和派生 Topic 关联。使用第 4 节的显式迁移入口；启动脚本不会代做迁移。
- 聚类使用构建该二进制的仓库内 `apps/comment-semantics/.venv`。必须在实际 runtime checkout 中准备，开发工作树里的 venv 不能证明 runtime 已具备依赖。
- 当前依赖锁针对 macOS 14+、Apple Silicon、CPython `3.14.5`。先安装匹配解释器和 `uv`，再以 `LINGGAN_SEMANTICS_PYTHON` 指向该解释器运行 `bash scripts/runtime/prepare-comment-semantics.sh --install`；随后 `--check` 必须通过。安装使用带哈希的固定 wheel 锁，服务启动不自动下载依赖。
- 没有匹配环境时，评论提取和已有结果仍可读；聚类记录 `semantic_runtime_unavailable`，不能宣称语义组织已经运行。更换平台需要重新制作并验证依赖锁，不能绕过环境校验。
- 在现有模型设置中配置可调用的研究模型；语义向量复用独立 Embedding 配置。研究设置中的语义组织子预算默认是 0，必须明确配置后才会产生向量/语义复核调用，并受全局日额度约束。规则比较同样需要回归子预算。
- 发布不自动开启新增研究、历史补齐或未知用量重试，也不批量重算历史评论。由保存的政策控制执行；额度不足保留待处理原因，旧结果不删除。
- 停机先等待与实际 worker PID 对应的 drain 回执；未结束的调用保留费用未知和恢复状态。共享发布仍须分别核对 Git revision、迁移台账、三个 PID 与实际页面。

隔离验证入口为 `scripts/test-comment-intelligence-postgres.sh`；本地聚类生产链专项为 `comment_semantic_organization_postgres`，需要先准备上述环境。容量报告只证明固定合成输入的资源和恢复行为，不代表真实评论研究准确率。
