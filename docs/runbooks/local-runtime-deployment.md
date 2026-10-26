# 本机常驻服务部署手册

> 状态: 权威当前
> 最后核对: 2026-10-26（新增 3001 并行部署候选；既有运行回执保留各节原核对日）
> 适用范围: AI-GEN1第四进程候选合同，以及本机既有三个 launchd 常驻服务（API / 巡检 worker / 媒体 worker）的运行来源、更新方式、故障处置与运行诊断
> 事实来源: Mog 于 2026-09-03 的明确要求「本地以 `/Users/moglenny/proma/linggan-intelligence` 为准，跟远端同步，不要到处复制」、当前 launchd 配置与实际运行验证、2026-09-17 更新入口用旧副本失败的实测（见 §3）、2026-09-21 按 COLLECTION-UPGRADE-001 S4 的实现与隔离库用例写成 §7（**样例为手工构造**，不是运行抓取；其余各节未重新实测，最后核对的口径见各节标注的日期）
> 冲突时以谁为准: 实际运行输出与 launchd 当前加载的配置；本手册不授予平台访问或迁移执行权限

## 2026-10-26 · 与内容工作台并行的 3001 部署候选

本机内容工作台继续使用 `127.0.0.1:3000`。Intelligence 的 `.env` 设置 `LINGGAN_LOCAL_PORT=3001`，PostgreSQL Compose 继续使用独立卷和 `127.0.0.1:55432`。`install.sh` 从 `.env` 生成 API plist；`migrate` 与首次 Comment Study 初始化只检查配置的 Intelligence 端口，不将另一服务的 3000 监听误认为本项目未停机。插件 `0.8.64` 的唯一目标也同步为 `http://localhost:3001`；构建 ZIP 不等于 Chrome 已重载。

本机首次准备先核对 Rust `1.95.0`、Node `24.13.0`、Docker Compose、PostgreSQL 16，然后运行 `setup-local-env.sh`、`dev-db.sh up` 和 `verify-development-environment.sh`。固定工具链不在登录 shell PATH 时，可在私有 `.env` 中设置 `LINGGAN_RUSTUP_BIN_DIR`、`RUSTUP_HOME`、`CARGO_HOME`、`LINGGAN_PI_NODE` 和 `LINGGAN_WEB_NODE`；具体路径不入 Git。构建、迁移与常驻服务应使用同一组路径。首次迁移与 Comment Study 基础表的顺序沿用本手册 §2；运行目录必须从干净、已核对的 `origin/main` 建立，不能把本机开发分支直接作为正式 runtime。

公网入口必须是另一个独立域名，Tunnel 指向 `http://127.0.0.1:3001`，保留浏览器原始 Host，并向本机 API 传递 `X-Forwarded-Proto: https`。域名和团队访问策略确定后，在私有 `.env` 同时设置精确的 `LINGGAN_PUBLIC_ORIGIN=https://<hostname>` 与 `LINGGAN_OPS_COOKIE_SECURE=1`，再逐项验收公网登录。API 仍只监听 loopback；公网 Host 仅在该配置下开放，跨站来源、未知 Host、插件写入、工位接口、旧采集授权/归档/租约/深化接口和公网 `/health` 拒绝访问。现有业务写接口尚未逐条接入角色和资源授权，因此公网首阶段仅允许 active admin 会话；非管理员仍可在本机按原有边界使用，正式团队范围开放须另补授权与审计。Cloudflare Tunnel 的路由与访问策略是独立配置，应用环境变量不会自动创建它们。

**2026-10-26 团队开放增量（覆盖上一段的管理员专属阶段）：**部署此增量后，已获授权的 9 位在用运营成员应可经 `intelligence.lingganboom.fun` 用内容工作台现有邮箱和密码登录。公网普通成员只开放发布记录、账号管理、每日数据的读取及原生录入，以及账户安全；发布采集同步的提交和回执本阶段仍限管理员。服务端读取按本人当前责任或魏东有效组范围限制，原生命令继续逐条核对写权限。另 12 位预置成员保持不可登录。部署后分别核对旧密码登录、本人录入、组长组内视图、跨组拒绝及公网其他模块拒绝；凭据导入是一次性快照，两系统后续改密不会自动同步。

上线验收分别核对 `127.0.0.1:3001/health` 的数据库与源码身份、登录/邀请/退出、管理员对各业务页面和媒体的授权、未登录与跨站负例、插件在本机 3001 的签到/回执，以及备份恢复和整版回退。巡检、媒体、AI 的禁用状态和真实平台/模型额度按原有合同保留；安装完成不自动宣称这些链路已运行。

2026-10-26 的首次本机库已按 `migrate → init-comment-study → migrate` 到 0166（账本 167 条）。使用容器内 PostgreSQL 16 `pg_dump -Fc` 经 `age` 加密的首装快照已用 `pg_restore --list` 检查，并恢复到一次性异库核对 167 条账本和团队表，随后删除验证库；身份密钥和快照都在仅当前用户可读的本机支持目录，不入 Git。该回执只证明本机快照可恢复；上线前仍需安排持续备份、密钥异地保管和整版回退窗口。

## AI-GEN1 / PR-1B 独立执行进程候选（未部署）

`linggan-ai-worker`现为第四个二进制候选。它不迁移、不采集、不处理媒体；启动只读核对0131–0141的迁移账指纹和必要结构，取得数据库/schema内的单例会话锁后才领取任务。物理进程运行不等于任务获准运行：全局/执行器/逐任务仍默认off，逐次核对配置、当前密钥、来源、外发与额度。`LINGGAN_AI_CONCURRENCY`只接受共享合同的4–8，缺省4；实际每个item有独立租约，实例独立每5秒心跳。

SIGTERM关闭新领取，保留已派发调用并只重试同次数据库结算。失效item的晚到响应只记最小用量，不能重新发布；持续结算失败保留未知占用和实际晚到用量，不报排空成功。原调用的固定deadline与回执总宽限合计采用共享合同最大超时+1+15；不能沿用旧installer的90秒作为AI排空上限。

文件收条由`LINGGAN_AI_DRAIN_ACK_PATH`指定，或写入`LINGGAN_SUPPORT_DIR/runtime-drain/ai-drain-ack`；先同目录临时文件/fsync/rename，再核对落盘。成功形状为`{state:"drained",processId,receipt:{instanceRef,generation,processId,sourceRevision}}`，generation是字符串；必须在DB实例已drained且无在途项后才产生。失败形状为`{state:"failed",processId,reason}`，没有成功receipt。合成进程的sourceRevision可以为unknown，不能冒充部署身份。部署端最终须将收条的实例/代次/PID/源码身份与同一数据库的当前实例及目标升级起点核对，文件或PID单项不足以放行代码替换。

**安装候选已接，尚未部署**：`com.linggan-intelligence.ai-executor`配置、DB/文件/PID/full源码身份核验、sync更新许可和prepare-migration四服务停机已接。等待与plist ExitTimeOut取新/现有资源合同可覆盖的宽限；source入口须为干净目标main。先捕获print-disabled，再排空巡检/AI，确认四服务退出才写原子起止revision许可；sync锁内重核当前数据库/实例/无在途、无会话锁再换代码，四binary及前端共同构建，身份写full SHA。数据库身份含cluster system identifier、database oid/name和schema；凭据只经环境传递，不进docker argv或许可。

安装保存原disabled：既有两个worker保持禁用，新AI label无先前意图时默认disabled且不bootstrap；不会执行enable或改数据库三层开关。旧flat permit不再被新sync接受。巡检已移除模型/embedding循环，仅保留10秒一次的确定性目录维护；其15秒有界事务完成后写自身原子收条，失败/收条不可写均非成功退出。旧业务API直调冻结、集中设置及0141旧字段删除已在PR-1B候选实现并隔离证明；完整交付/CI状态见[PR-1B实施包](../plans/active/ai-gen1-pr1b-foundation.md)，不能只凭本节发布。

开发`local-runtime.sh serve`源码已接第四进程。停止时核对两个自有子进程退出和数据库；开发态unknown身份的收条只产生local_drained，不能成为发布许可。失败保留其他自有进程与收条并以失败退出。证明覆盖精确cleanup函数的四个dummy子进程控制，未运行含migrate的完整serve，更未操作共享服务。旧patrol模型/强杀测试随旧owner退役；新AI信号/晚到/持续结算失败已证明，实际强杀→重启接管专项已在owned PG/loopback证明；Mac常驻安装与共享迁移仍未执行。

本轮通过替身命令安装演练与owned PostgreSQL/真实binary核验，不代表Mac launchd、共享数据库备份/迁移或部署已完成。此节不授予这些动作；下文三服务回执保留原适用版本。

## 2026-10-19 · WEB-REACT-001 全站收尾运行验证

本节是当前收尾包的执行合同，结果以[收尾矩阵](../design/acceptance/web-react-001-closeout.md)和实际发布回执为准。正式运行源码只在干净的 `runtime-main`，共享开发目录和其他工作树保持原样。本包不新增schema，不运行migration；巡检/媒体LaunchAgent当前禁用且未加载，部署不改变该边界。

1. 从已审查PR的精确head取得实际CI结果，合并后记录完整main SHA。先记录3000 listener PID/cwd/executable、runtime/frontend revision、迁移账本、研究/预算策略哈希、running invocation与业务计数。
2. 在canonical runtime使用受控 `zsh scripts/runtime/install.sh` 同步和同revision构建。保持现有`.env`。若launchd登记失败，先核对已写plist合法、服务是否加载及listener；只恢复确实缺失的现有API plist，不盲目重跑installer或启用禁用的worker。构建完成不等于运行完成。
3. 新listener必须来自canonical runtime；`/health.webApp`的revision/runtimeRevision与main一致、数据库READY；17个登记路由与子视图实际打开。未知路由和旧资源404，API不能被SPA入口吞掉。
4. 正式离线演练前确认无在途调用，备份API plist到登记private目录。临时只为API启动进程应用macOS网络沙箱：禁止站外出网，仅允许当前PostgreSQL loopback与Docker Unix socket，设置Cargo/npm离线复用。先以本地连接成功、站外连接被拒的正反例验证限制，再停止/重新加载唯一API服务；核对新PID、同revision、health与页面。此证据称“API进程对站外断网重启”，不宣称整机断网。
5. 整版回退必须在没有新稿件/模型写入的窗口进行，确认前后schema兼容；停止API，将干净runtime切到已核实的上一完整revision，在同一出网限制下重新构建并启动，核对后台、前端和页面一起回退。不得用新后台配旧页面，不运行down migration，不恢复旧页面兼容路由。362dad3f不支持新版非空材料准备字段和V3图页模板：回退演练仅在升级前作品上进行，且必须在新版首次写入前完成；新能力写入后不得直接降到该旧版本，应使用能读取当前文档合同的整版修复revision。空集合/None不额外写新字段只保护未使用新能力的旧V1作品，不宣称任意新版稿件可由旧版读取。
6. 演练后恢复原API plist，通过官方installer回到最新main，重新确认版本、服务边界、迁移账本及策略/计数，再执行正式业务验收。私有原日志、截图、回执保留，不把预检或隔离库结果写成正式重启/回退成功。


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
./scripts/init-comment-study.sh          # 仅首次安装：初始化 clean-study 基础 schema
./scripts/local-runtime.sh migrate       # 应用 clean-study 增量迁移
./scripts/runtime/install.sh             # 建运行目录、写 launchd、启动三个服务
```

若数据库已经有完整 clean-study 基础表，跳过 `init-comment-study.sh`，只运行一次 `migrate`。如果首次 `migrate` 因 Comment Study 基础表缺失而停止，先运行初始化脚本再重试。初始化脚本会拒绝任何已有或部分存在的 `linggan_comment_study_*` 表，不执行 reset，也不恢复旧研究派生数据。

`install.sh` 幂等：已装好时重跑只刷新 plist 并重启。它**不**跑迁移、不装依赖、不碰远端环境。

## 3. 更新方式

CI-AUTO-004 源码将版本切换改为先等待评论调用落账。以下是本包发布后的更新合同，不表示当前常驻进程已经使用这版协议。

```bash
./scripts/runtime/install.sh
```

`install.sh` 先请求巡检 worker 停止领取新模型任务，等待它将当前调用和用量落账，并核对与 PID 对应的退出回执。成功后才生成绑定起止 revision 的更新许可，再同步、构建并重新加载三个服务。未确认退出时停止更新，不把未知调用费用当作零。

**入口脚本必须来自与 `origin/main` 同步的副本。** 2026-09-17 实测踩过：开发目录停在 `c94a3ec`，那份 `install.sh` 里还留着一段「兼容仍会构建已退休二进制」的补丁（`install.sh:168-177`），`cargo build --bin linggan-comment-worker` 当场报 `no bin target named ...` 并以 101 退出。**失败点在 drain 之后、重启服务之前**——巡检 worker 已被 `bootout` 且没有装回去（`launchctl list | grep linggan` 只剩两个 label），另外两个服务仍是旧进程。它不是「什么都没发生」的失败，而是「停了一个、旧了两个」。

所以更新前先确认入口副本不落后，或者直接用运行目录里那份：

```bash
git -C ~/Library/Application\ Support/Linggan\ Intelligence/runtime-main log -1 --date=short --format='%h %ad %s'
~/Library/Application\ Support/Linggan\ Intelligence/runtime-main/scripts/runtime/install.sh
```

`install.sh:152-159` 对「安装入口就在运行 worktree 里」的幂等场景有显式处理，`install.sh:161-166` 也写明「旧运行目录可能含已退休的二进制，因此从源 checkout 跑目标 revision 的同步器」。无论用哪份入口，跑完必须核对三件事：`launchctl list | grep linggan` 是**三个** label、`git -C $runtime_dir log -1` 等于目标 revision、三个进程的启动时间是刚才而不是上一次部署。

启动时 `scripts/runtime/sync.sh` 依次做：

1. 取同步锁（三个服务同时启动时串行化）；
2. `git fetch origin main`；需要改变 revision 时核对并消费匹配的 worker 更新许可，之后才切换；
3. 检查迁移台账，有未应用的迁移就**拒绝启动**（见下）；
4. 在锁内构建三个二进制；
5. 在锁内准备网页前端（`scripts/runtime/prepare-web.sh --build <revision>`，WEB-REACT-001 起）；
6. 释放锁，各服务 `exec` 自己的二进制。

拿不到网络时不会让服务起不来：fetch 失败会记录一行并继续用当前 revision。

### 3.1 网页前端（WEB-REACT-001 / DEC-0010 起）

迁到 React 的页面（当前只有 `/settings/models`）由 API 读取**同一个运行目录**里的 `apps/web/dist` 提供。`prepare-web.sh`：

- 用与 Pi Adapter 相同的固定 Node（按 `.nvmrc` 解析绝对路径，不依赖 launchd 的 PATH；可用 `LINGGAN_WEB_NODE` 覆盖）；
- 该 revision 已构建过（`dist/build-info.json` 的 revision 相同）就直接复用——**离线重启不需要网络**；
- 只有 `package-lock.json` 变化时才 `npm ci --prefer-offline`；本机 npm 缓存里有对应的包时断网也能装上，否则需要网络（与 cargo 首次拉取新依赖同理）；
- 构建失败与 cargo 失败一样**拒绝启动**，完整输出在 `runtime-build/sync-build.log`。

运行中的 API 还有第二道闸：`dist/build-info.json` 的 revision 与身份文件里的 revision 都已知却对不上时，React 页面返回 503（`web_revision_mismatch`），JSON 接口照常工作；产物不存在时返回 `web_dist_missing`。核对方式：

```bash
curl -s http://127.0.0.1:3000/health | python3 -c 'import json,sys; print(json.load(sys.stdin)["webApp"])'
```

`state` 应为 `ready`，`revision` 与 `runtimeRevision` 相同。回退仍以整个 revision 为单位：切回旧 revision 后 `sync.sh` 会为它重新准备前端（或复用已有构建）；不保留旧页面作为单页回退手段。

## 4. 迁移仍然不由服务执行

这条约束从旧模式沿用，理由没变：**一个开机自启的服务不该顺手改数据库结构**。

但代码现在会自动跟到 main，所以同步脚本必须主动检查——否则新代码撞上没跑的迁移，
报出来的是一堆 SQL 错误，而不是一句「该迁移了」。检查到有未应用迁移时服务拒绝启动，
日志里会列出是哪几个，处置方式固定：

```bash
cd /Users/moglenny/proma/linggan-intelligence
./scripts/runtime/install.sh --prepare-migration
./scripts/local-runtime.sh migrate
./scripts/runtime/install.sh
```

`--prepare-migration` 会先通过旧巡检 worker 的 drain 回执，再停止 API 与媒体 worker；该模式不切换代码、不重启服务。持久数据库的 `migrate` 会检查三个 launchd 服务、对应进程和 3000 端口都已停止，否则拒绝继续。增量迁移成功后再用普通 `install.sh` 同步并启动三个服务。不要在服务运行时直接调用 `migrate`。

**已知边界**：开机时 API 可能比 Docker PostgreSQL 先起来。此时读不到台账，脚本会记录
「读不到迁移台账」并**跳过检查继续启动**——硬拦会让服务在数据库没起来时永远起不来。
因此开机后第一次启动不保证迁移检查生效；手工重启一次即可获得完整检查。

### 回复 TaskSpec v2 的一次切换（COLLECTION-REPLIES-CONTRACT-001）

历史回复重判涉及同一处置表上的 0105 和 0106 两次迁移：0106 允许旧任务超出 `maximumQuota` 的合同失配原始原因参与同一逐条重判，不建立第二套准入账本。迁移必须在新版服务启动前完成。

本次升级把唯一 Browser Producer TaskSpec 切为 v2，服务端最低插件版本为 0.8.57。先停止新工单领取，核对未到期租约与浏览器 durable outbox 中的 v1 交付；让已入队材料按原合同完成交付，再同时切换 API、巡检 worker 与插件。不得让旧插件领取 v2 任务，也不得让新版插件把旧 outbox 的 v1 TaskSpec 当成新任务重发。恢复领取前核对插件安装版本、三进程 revision、迁移台账与一条真实 Task→Attempt→Package→Receipt→disposition 链；再单独执行 `linggan-material-reproject --requalify-replies` 预览，核对候选数并安排低负载窗口后才由获授权的运维人员执行 `--apply`。该命令一次读取候选包并在单事务内重判，是历史重判的唯一受控写入入口；数据库 trigger 约束一次写入的形状，身份与逐条关系校验由命令完成。

2026-09-28 的只读盘点显示共享库没有未到期且未释放的租约；历史 v1 租约任务仍有 `pending=852`、`in_progress=26`，均须按租约当前有效性判断，不能按状态字符串直接当作正在运行。最近 24 小时报到的两个安装仍为 0.8.56；浏览器 outbox 是否仍有待发送的 v1 envelope 尚未核对，因此候选代码、迁移和 0.8.57 ZIP 不构成已完成切换的证明。

## 5. 三个服务

| 服务 | launchd label | 二进制 |
|---|---|---|
| API（3000 端口） | `com.linggan-intelligence.local-runtime` | `linggan-api` |
| 巡检调度 | `com.linggan-intelligence.patrol-worker` | `linggan-worker` |
| 媒体处理 | `com.linggan-intelligence.media-worker` | `linggan-media-worker` |

三者由同一个 `scripts/runtime/launch.sh <binary>` 启动、共用同一份 `sync.sh`。更新后仍须分别核对三个 PID 和构建 revision；共用脚本不证明当前存活进程已经同步。

**单次采集由 API 自己推进**（EVIDENCE-INSPECTOR-RECORDS-001，Mog 2026-10-23；候选，合并部署前不生效）：`linggan-api` 启动后每 15 秒调用一次 `collection_run_progress::advance_runs`，只推进采集页「单次采集」与证据库「评论深采」建立的 `collection_run`——按已接纳交付冻结范围、准入下一段，不访问平台、不调模型，也不运行巡检、建档补采等其它调度。所以巡检 worker 停用时，用户发起的单次采集仍会排出下一段并等工位认领；巡检 worker 若恢复，两边同时推进也安全（每次采集一把咨询锁，抢不到就跳过）。它和 worker 一样受 `LINGGAN_COLLECTION_UPGRADE_PHASE=governance` 门控，日志落在 `api.err.log`（`collection run advancement failed`）。

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

## 7. 运行诊断：一个 tick 号串一条链

巡检 worker 每轮 tick（60 秒扫一次）有一个号，落在 `collection_scheduler_run.scheduler_run_ref`。
同一个号写在它的步骤行、这一步排出的目标级决定、以及那些工单上；日志里的事件行用 `tickRef`
带着它。**一条链断在哪里，从这一个号出发就能查出来，不需要累计任何日志。**

三个查询（`psql`，把 `<run>` 换成 run 号；工单号由上一条给出）：

```sql
-- ① 这一轮四步各自的结果。失败与「没轮到」在这里分开，失败带的是受限类别，不是报文。
SELECT step_key, outcome, error_class, skipped_reason,
       considered_count, produced_count, skipped_count
  FROM collection_scheduler_run_step
 WHERE scheduler_run_ref = '<run>'
 ORDER BY started_at;

-- ② 巡查这一步把哪些目标排成了活、各自给了什么理由。
SELECT target_ref, outcome, reason_code, work_order_ref
  FROM collection_scheduler_target_decision
 WHERE scheduler_run_ref = '<run>';

-- ③ 排出的工单此刻在哪条队列、什么时候该跑。
SELECT work_order_ref, target_ref, queue_state, dispatch_lane, scheduled_for
  FROM collection_work_order
 WHERE work_order_ref = '<工单号>';
```

同一轮在 `runtime-logs/worker.out.log` 里的三行（一行一个 JSON 对象）：

```json
{"ts":"2026-09-21T09:00:00Z","service":"worker","revision":"9a1c2f4","event":"tick","tickRef":"<run>","outcome":"failed"}
{"ts":"2026-09-21T09:00:04Z","service":"worker","revision":"9a1c2f4","event":"tick_step","tickRef":"<run>","stepKey":"media_acquisition","outcome":"failed","errorClass":"sqlstate_42703","durationMs":12}
{"ts":"2026-09-21T09:00:05Z","service":"worker","revision":"9a1c2f4","event":"tick_step","tickRef":"<run>","stepKey":"patrol","outcome":"ok","durationMs":41,"considered":1,"produced":1,"skipped":0}
```

**这三行是手工构造的脱敏样例，不是从运行日志里抓的**：形状按实现写（`<run>` 是省略的号），
用来对照字段，不构成任何一次真实故障的证据。同一条链在隔离 PostgreSQL 上有可复现的用例
（`crates/evidence/tests/scheduler_tick_postgres.rs` 的
`one_tick_ref_strings_the_failure_and_what_it_queued_without_reading_the_logs`）。

读法：`outcome` 是「这一步跑完没有」，`failed` 与 `skipped` 是两件事——`failed` 是跑了但坏了
（带 `errorClass`），`skipped` 是这一步自己的表不在、**没轮到**（带 `reason`，如
`schema_unavailable`）。一步失败不连坐其余三步：上面例子里媒体投影坏了，巡查照样把活排了出去。
计数只在 `outcome="ok"` 的行上出现；失败、没轮到、以及**开始了却没写完结（进程被杀）**的行
一律没有计数——「数过了是零」与「没数过」不能互相冒充。库里 `outcome IS NULL` 的那一行就是
最后这种情况，它既不写成 `ok` 也不写成 0。

### 7.1 字段白名单

事件只能带这 13 个字段（`crates/evidence/src/runtime_event.rs` 的 `EVENT_FIELD_WHITELIST`，
结构体里没有「随手塞一个键」的入口）：

| 字段 | 含义 | 边界 |
|---|---|---|
| `ts` | 事件发生的时刻 | **进程时钟**（UTC、秒），不是数据库时钟 |
| `service` | 谁在说话 | `worker` / `media-worker` |
| `revision` | 这份进程是哪个部署 | 读不到写 `unknown`，不拿别的东西冒充 |
| `event` | 事件类型 | `startup` / `readiness` / `tick` / `tick_step` |
| `tickRef` | 这一轮 tick 的号 | 与 `scheduler_run_ref` 是同一个值，串证的入口 |
| `stepKey` | 哪一步 | 四步之一；`media_acquisition` 这类受限词 |
| `outcome` | 这一步（或这一轮）的结局 | `ok` / `failed` / `skipped` |
| `reason` | 没轮到的原因 | 受限码 `[a-z0-9_]{1,32}`，如 `schema_unavailable` |
| `errorClass` | 失败的类别 | 受限码，如 `sqlstate_42703`；**不是报文** |
| `durationMs` | 这一步花了多久 | 进程时钟测出来的毫秒数 |
| `considered` / `produced` / `skipped` | 这一步数过的三个计数 | 只在跑完的步骤上出现 |

两条纪律：**引用只到 tick 号为止**——目标、工单、租约、任务、提交这些引用住在数据库里，
把副本抄进日志只会多出一份会漂移的东西，也会让「什么算敏感」在第二个地方重新判一遍；
**理由是受限码**——不规则形状（空格、斜杠、问号、中文、连接串）一律记 `unclassified`，
不把散文按字符挑成一个「看起来像原因码」的词。

页面结构自检的快照另有自己的一份字段表（`crates/evidence/src/selector_health.rs` 的
`SELECTOR_HEALTH_SNAPSHOT_FIELDS`，与插件 `src/shared/selectorHealth.js` 里的同名常量是同一份
合同的两侧），一条平台记录九个键：`platform`、`pageType`、`capability`、`checkedAt`、
`verifiedAt`、`checkedCategories`、`missingCategories`、`staleCategories`、`failureCounts`
（服务端比插件多最后一项：连续次数由本机留存层补上，页面看到的那一次里没有「连续几次」）。

快照里**没有**这两样，也不要去找：`pluginVersion`（报到自述里本来就有，存进
`plugin_installation.plugin_version`——同一件事不能有两个答案）、`ruleVersion`（今日它就是
`verifiedAt` 的别名，另起一个名字会让人以为存在一套独立的规则版本）。

白名单不是承诺，是测试判据：Rust 侧 `an_event_carries_only_whitelisted_fields` 与
`every_field_the_event_can_carry_is_whitelisted_and_used` 断言事件的字段**逐字等于**那张表
（多一个、少一个都变红），串证用例再从真实 tick 路径上验一遍；
`nothing_outside_the_field_whitelist_survives_into_the_snapshot` 断言一份夹带了选择器串、
DOM 文本、带签名的地址的载荷收下来之后那些键一个不剩。插件侧另有 node 用例断言出门前
只带快照字段。

### 7.2 这一节不证明什么

- 样例是**描出来的**，不是抓来的：它证明字段表长得对、查得到，不证明任何一次真实故障被这样
  串起来过（那要有一次真实提交，属真实平台访问，未授权）；
- 只覆盖本机三个常驻服务，不含部署机（Mac mini）与任何远端环境；
- 日志只带引用，**不带目标与工单的内容**：要回答「排出给谁、排的是什么」必须真的查库；
- 库里的步骤、决定与工单是采集链的事实，日志只是进入那条链的入口。两者不一致时以库为准，
  并把不一致本身当作一次待查的缺陷。


## 8. 本手册不证明什么

- 不证明部署机（Mac mini）或任何远端环境采用同一模型——这里只描述本机；
- 不授权服务自行执行迁移、访问平台或修改数据库；
- 不保证开机首次启动时迁移检查一定生效（见第 4 节的已知边界）。


## 9. COMMENT-RESEARCH-RESET-001 V1 的发布前条件

此段规定当前唯一评论研究路径；CI-AUTO-004 的 daily/Task B/P4/本地 Python 聚类实现是历史记录，不是 runtime 依赖。

1. 先在交付 head 执行 `bash scripts/test-comment-research-postgres.sh`、`npm test --prefix apps/pi-adapter` 与 `cargo check -p linggan-intelligence -p linggan-api -p linggan-worker`。前者会在随机隔离 PostgreSQL 中从历史 migration 完整应用到 `0085`，验证旧研究派生被显式退役、V2 Atom→Stable Problem 的受限归并、归属修正会生成新 derivation head、冻结 Run 仍可完成而整版旧结果撤出可读投影；它绝不访问开发库。
2. V1 不需要 Python/HDBSCAN/Leiden runtime。Pi adapter 只需在实际 runtime checkout 执行 `bash scripts/runtime/prepare-pi-adapter.sh --install`，再用 `--check` 验证固定 Node/npm 依赖。
3. 在运行目录更新前，通过本节既有受控 `install.sh` 取得 worker drain 回执。旧 worker 的在途调用不能在删除旧表期间继续写入；未结束调用必须保留 invocation ledger 的未知用量事实，不能计作零。
4. 更新后的代码仍不能自行迁移。取得 drain 后，从 canonical main 使用第 4 节的 `./scripts/local-runtime.sh migrate` 显式应用 `0068`–`0085`。`0069` 是终态删除 migration：清除旧研究派生和可能已存在的早期 V1 派生结果，不使用 `CASCADE`，并保留 Raw Comment/Evidence、作者归属、来源资格、通用模型连接/配置、embedding 设置和 invocation ledger；`0070` 固化 V1 derivation identity/read boundary：新 Run 选当前 head，冻结 Run 保留原输入可读性，不迁移或恢复任何旧研究结果；`0084`/`0085` 只增添 V2 Frame、closed-world Resolution 和可恢复 pair checkpoint，绝不重写既有 Problem 或原始评论。
5. migration 后安装并刷新服务，逐项核对 exact Git revision、migration ledger、三个 PID、`/health` 和 `:3000` 的 V1 页面。旧 route/page 不应再可访问。
6. 发布、保存连接或保存 policy 均不会调用真实模型；连续排程没有入口且保持关闭。只有用户在页面主动点击“开始研究”才会冻结 V1 Run 并发送已获许可的评论。

真实 provider 的语义质量与 Mog 业务验收不由上述基础设施步骤推断。


## 8. COLLECTION-UPGRADE-001 分阶段刷新与停止新访问

先应用 0096–0101，再从已合并 main 的最新脚本执行部署。部署源 `.env` 明确设
`LINGGAN_COLLECTION_UPGRADE_PHASE=recovery`；缺失/未知值同样为 recovery。`install.sh`
会同步该配置并受控 drain/restart。核对 `/health.collectionUpgradePhase`、readiness、
导航准备契约和部署 revision 后，才将部署源配置改为 `governance`，再次受控刷新。
recovery 下旧包的 Attempt 重放与 Submission 保持开放，不应清空浏览器数据库。

插件从本次 0.8.55 release-manifest 指定 ZIP/源码 dist 切换，保留同一个扩展 ID 与用户数据。
核验加载路径、版本和新的工位报到，不能把 ZIP 已生成写成实际插件已升级。

有问题时使用**新版二进制切回 recovery**：它能识别新台账、准备身份与 input_blocked。
不得重置远端 main，也不执行 down migration。旧二进制会将 input_blocked 误读为排队；
未经新库兼容实证不得切回旧二进制。恢复阶段的 PostgreSQL 用例证明禁止新 claim 时旧包仍可接纳。

历史处置工具默认只读（命令运行环境需设置正确的 `LINGGAN_LOCAL_DATABASE_URL`）：

`COLLECTION-REPLIES-CONTRACT-001` 的回复历史恢复使用同一个 `linggan-material-reproject` 运维入口，
但只处理旧任务把 `replyExpandLimit` 放入 `target`、且原处置恰为
`task_package_contract_mismatch` 的 replies Package。先执行 `0105` migration，再预览；
预览只读。`--apply` 在一个事务内用当前任务/包绑定与逐条回复关系 validator 重判，
将原始隔离判定留在同一 disposition 行的 `initial_*` 字段，并调用既有 Material 投影。
身份不符、已有投影或不完整的原包均不得恢复；关系不合格的记录不得准入，同包合法回复仍可恢复。该操作不扫描实时提交流程的历史，
不改不可变 Task、Attempt、CapturePackage 或 Receipt。共享库执行需受控发布授权和运行态核验。

```bash
cargo run --locked -p linggan-worker --bin linggan-material-reproject -- --requalify-replies
cargo run --locked -p linggan-worker --bin linggan-material-reproject -- --requalify-replies --apply
```

```bash
cargo run --locked -p linggan-worker --bin linggan-collection-repair > /tmp/collection-repair-preview.json
# 默认只展示最近 100 个对象；truncated=true 时可按原 taskId 精确查看：
cargo run --locked -p linggan-worker --bin linggan-collection-repair -- --task-id <UUID>
# 经逐项批准后的显式应用；UUID/hash 必须来自同一原始预览：
cargo run --locked -p linggan-worker --bin linggan-collection-repair -- --apply /tmp/collection-repair-preview.json --batch-id <UUID> --preview-hash <SHA256>
```

默认预览只 SELECT；应用逐项重新核对并输出批次/哈希/结果。每批最多 100 个对象，
每项锁等待 2 秒、语句 5 秒。冲突跳过而不强制重试，需重新预览；已成功项重放无额外事件。
仅停止有明确作用域、缺输入且尚无任何在途材料的 pending 通道。旧 released/in_progress、
已开始或有准备会话的对象不改写；本地 terminal/缺历史身份材料是 NOT_OBSERVED，需在原浏览器
另作逐项核验，不得据服务端预览批量复活。输入历史未知的失败预算不追溯猜造。
保存预览与应用 JSON 作为运维回执，原 Package、Receipt、失败历史均保留。
