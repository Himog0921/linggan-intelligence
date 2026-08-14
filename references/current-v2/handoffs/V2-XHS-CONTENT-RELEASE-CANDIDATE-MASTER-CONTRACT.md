# V2 XHS Content-only Release Candidate：单次授权连续执行总控合同

> 文档性质：不可变执行合同。执行代理开始后不得修改本文件。
>
> 总控主线：Codex `/root` 最终独立审核。
>
> 用户授权：一次性完成所有可在上线前安全执行的工作；内部 checkpoint 不是审批点，不得中途等待用户或主线确认。
>
> 自动执行终点：`READY_FOR_OPERATOR_CUTOVER` 的 XHS Content-only V2 Release Candidate。
>
> 明确不属于自动执行：push、merge、deploy、正式测试库/生产库写入、九工位真实升级、真实线上 48 小时观察、Release-C 物理删除。

---

## 0. 执行摘要

本任务不是继续拆小工单。执行代理必须在一次长任务中，按依赖顺序连续完成所有可执行阶段，自行修复范围内问题，自行运行攻击性测试和全量门禁，最后只交付一个覆盖写回的 handoff，交由主线统一审核。

目标纵向链：

```text
XHS 插件六类采集
  → execution / manual_import / recovery（migration 首期仍无 caller）
  → 唯一 EvidenceIngress
  → 受控 Evidence Reader + EvidenceAccessAudit
  → durable V2 后台推进
  → B2 Normalization / ContractEvaluation
  → B3 Content Canonical / Media / Projection
  → 唯一 Content Projection Read Service
  → Material 页面 Content-only 读取候选
```

首个上线切片只含：

- XHS Content；
- current + accepted + visible Content Projection；
- observed image/video 媒体事实；
- 中央封面规则；
- 当前接受观察中已验证的原始分享链接；
- Material 页面唯一读取候选。

本任务不以“整个 V2 全域完成”为目标。以下明确后置，不得猜造以追求完成率：

- Author Projection；
- Comment / CommentObservation；
- Metric；
- B6 强一致 PresentationRequirement / PresentationReceipt；
- `absent/unavailable` 媒体 producer 事实；
- `live_photo` 物理映射；
- Douyin；
- Release-C 物理删除。

---

## 1. 固定点、输入与真实状态

### 1.1 工作台

- worktree：`/Users/gongyong/Services/content-workbench/v2-b3-projection-readiness`
- branch：`v2/b3-projection-readiness`
- 必须精确等于的 HEAD：`744deeeeb455d109565e4578679f7c4c7c8d9318`
- 当前唯一允许存在的未提交文件：
  `docs/code-review/v2-b3-caller-read-cutover-source-audit-2026-08-12.md`
- 该文件开始时必须精确校验 SHA-256：
  `0d8543a0f492873ea27af504d2fb621c784cda5ed3196b74dd399ab90f721dfa`
- 注意：文件标题仍写 `B3-A-04-R2`，不得仅根据聊天称其为 R3；只能称“最终审核通过的 A-04 审计文件”。

### 1.2 插件

- repo：`/Users/gongyong/Services/linggan-boom`
- branch/status 基线：`main...origin/main [ahead 6]`，工作树 clean
- 必须精确等于的 HEAD：`c22fa1b160a74b741bf56a52ebab0e6cfed9eb1a`

### 1.3 环境基线

- 2026-08-12 只读核对：磁盘可用约 18GB；最低门禁 5GB。
- Docker 容器 `topic-dashboard-local-postgres`：`running true`。
- 正式测试库：`127.0.0.1:54329/content_workbench_local`，严禁写入、迁移、reset、db push 或测试。
- `.env.local` 会覆盖普通 `DATABASE_URL=...`；不得以环境变量表面值推断目标库。

### 1.4 不可信输入

以下均不能作为完成证据：

- 对话里的交付摘要；
- 旧 `/private/tmp` handoff；
- Agent 记忆或上下文摘要；
- 类型定义、mock、接口 `ok:true`；
- 旧证明库结果；
- 测试文件存在但未执行；
- 历史全量测试数字；
- 文档写了 `CLOSED` 但代码/数据库未证明。

任何事实都必须从本次固定源码、schema、migration、真实命令输出和全新隔离库重新取证。

---

## 2. 权威资料及优先级

开始时完整读取，不得只读摘要：

1. `AGENTS.md`
2. 本合同
3. `docs/architecture/v2/00-contract.md`
4. `docs/architecture/v2/01-decisions.md`
5. `docs/architecture/v2/02-model-contract.md`
6. `docs/architecture/v2/03-validation-matrix.md`
7. `docs/architecture/v2/04-blocker-ledger.md`
8. `docs/architecture/v2/05-execution-plan.md`
9. `docs/architecture/v2/06-v2-operating-contract.md`
10. `docs/architecture/v2/07-evidence-ingress-release-b-contract.md`
11. `docs/architecture/v2/08-xhs-collection-contracts.md`
12. `docs/architecture/content-workbench-v2-design-freeze.md`
13. `docs/code-review/v2-b3-caller-read-cutover-source-audit-2026-08-12.md`
14. `docs/guides/coding-standards.md`
15. `docs/guides/testing.md`
16. `docs/governance/document-maintenance-protocol.md`
17. `docs/governance/file-placement-standard.md`
18. `docs/TODO.md`
19. 插件仓 `AGENTS.md`、`MESSAGE_PROTOCOL.md`、`REPLICATION_BLUEPRINT.md`、`DATA_MODEL.md`、`AI_READY_DATA_CONTRACT_V1.md`、`docs/decisions/index.md`、`progress.txt`

Phase 0 必须用 `test -f` 对上述每个真实路径逐项验证。插件实际路径应以仓库 `rg --files` 为准；当前已知 `REPLICATION_BLUEPRINT.md` 位于 `docs/REPLICATION_BLUEPRINT.md`，不得凭历史路径猜测。任一权威输入缺失只冻结依赖 lane，不得静默跳过。

判定分为两轴，禁止混用：

```text
当前现实轴（回答“现在实际发生什么”）：
真实源码 / schema / migration / 数据库行为 > 文档与报告

目标规范轴（回答“V2 必须成为什么”）：
本合同明确授权边界
  > 00 / 01 / 06 / 07 / design-freeze 的已确认规则
  > 02 / 03 / 04 的当前来源与 blocker
  > 现役 V1 行为与历史文档
```

“代码是现实”不等于“现役 V1 代码自动成为 V2 规范”。例如审计发现 V1 fallback 时必须报告它确实存在，但目标实现必须删除/隔离它，不得继承为 V2 语义。

### Suggested skills

执行代理应在开始时读取并遵循以下技能：

- `implement`：按合同连续实施；
- `tdd`：每一纵向阶段先 RED、后 GREEN；
- `diagnosing-bugs`：真实失败时按假设排序定位，不凭直觉修改；
- `code-review`：最终从固定点做 Spec / Standards 双轴自审。

技能不得覆盖本合同的安全和架构边界。

---

## 3. 一次性授权信封

### 3.1 已确认，无需再问用户

- `DR-B3-CALLER-001 = 方案 A`：Evidence 保存成功后，由独立、durable、幂等、可重试后台链推进 B2/B3。
- 采集成功、Evidence 成功、Derived 成功、Projection 可见是不同事实。
- 单轨：生产任一时刻只有一条新数据写入路径和一条页面读取路径。
- 禁止双写、双读、fallback、字段级择优、兼容路径承接新数据。
- 历史关系不伪造；旧历史允许不可用或退役。
- execution/manual_import/recovery/migration 四类都属于 EvidenceIngress 合同；首期 migration 无 caller。
- manual_import/recovery/migration 不得伪造 ExecutionJob、TaskAttempt、Station 或 lease。
- XHS 首期，Douyin 排除。
- 页面只读 Projection；AI/业务服务不能直接读 Evidence。
- accepted 不等于 active；无生命周期来源时不能猜 active。
- accepted A→accepted B：先隔离旧 Projection/媒体关系，新 Projection 原子完成后再 visible。
- 打开原文使用当前接受观察中 Evidence 提供的原始分享链接，不建设额外 Action 平台。
- 封面：平台明确封面 → 同一接受观察首张已证明图片 → 不可用。
- 同物理媒体可以服务多个 Canonical slot；一个 MediaItem 不得吞掉多个业务语义。
- observed 媒体可进入首切；`absent/unavailable` 不得从缺候选、下载失败或合同 slot 推断。
- `live_photo` 未有物理映射时 fail closed。
- Author/Comment/Metric 不阻塞 Content-only 首切，但不得夹带实现。

### 3.2 执行代理可自行决定

仅限不改变业务事实、权限、持久化语义的局部技术选择：

- 私有函数拆分；
- 文件内命名；
- 测试组织；
- 小型类型抽取；
- 在已有明确角色/合同内的 SQL 组织；
- 不改变外部行为的性能优化。

以下不是局部技术选择，禁止自行决定：

- 新模型/新表是否成为事实 owner；
- durable handoff 使用哪种物理语义；
- 状态值域或终态；
- GRANT/REVOKE 的业务授权 owner；
- 新的 fallback 或兼容路径；
- 历史回填规则；
- 页面强一致归属；
- 生产切流、部署、正式迁移、Release-C 删除。

### 3.3 本次允许的写操作

- 修改两个本地仓库中本合同明确范围内的源码、schema、migration、测试、脚本、文档；
- 创建全新隔离数据库；
- 创建可复现的本地插件构建包；
- 按 checkpoint 创建本地 Git commit；
- 修复执行过程中独立 review 发现的范围内问题；
- 更新 handoff 文件。

### 3.4 禁止动作

- push、merge、PR、deploy；
- 修改生产或正式测试数据库；
- `prisma migrate reset`；
- 对 `content_workbench_local` 运行 `migrate dev`、db push、migration、seed、回填或测试；
- `--accept-data-loss`；
- 删除证明库；
- 修改或删除用户无关改动；
- `git reset --hard`、`git clean`、破坏性 checkout；
- 为通过测试降低断言、跳过用例、吞异常或修改治理检查器；
- 未经来源新增第二 Evidence/Media/Queue 平台；
- 在本次候选中物理删除旧表/旧字段/历史数据；
- 把部署、九工位升级、正式迁移或 48 小时写成已完成。

---

## 4. 长任务状态机与不中断规则

每个 Phase 在 Phase 1 manifest 中声明 `phaseKind=documentation|implementation|verification`。implementation 使用完整状态机：

```text
NOT_STARTED
→ SOURCE_AUDITED
→ RED_PROVEN
→ IMPLEMENTED
→ DIRECTED_GREEN
→ ATTACK_GREEN
→ FULL_GATE_GREEN
→ SELF_REVIEWED
→ CHECKPOINT_COMPLETE
```

documentation 使用 `NOT_STARTED→SOURCE_AUDITED→SELF_REVIEWED→CHECKPOINT_COMPLETE`；verification 使用 `NOT_STARTED→RED_PROVEN→ATTACK_GREEN→FULL_GATE_GREEN→SELF_REVIEWED→CHECKPOINT_COMPLETE`。不适用状态只能标 `NOT_APPLICABLE` 并引用本合同依据；禁止为了过门伪造 RED/IMPLEMENTED/GREEN。manifest 必须拒绝非法跳转和无理由 N/A。

checkpoint 是内部质量门，不是审批点。达到 `CHECKPOINT_COMPLETE` 后：

1. 整体覆盖 handoff；
2. 可创建本地 checkpoint commit；
3. 立即进入下一 Phase；
4. 不向用户或主线请求继续授权。

### 4.1 RED-1：范围内可修复

类型错误、migration 语法、validator 漏洞、事务部分成功、enqueue 失败误报成功、并发 bug、测试 helper 假通过等：

1. 保存真实 RED；
2. 根因排序；
3. 补最小可证伪测试；
4. 修复；
5. 重跑本阶段全部门禁；
6. 继续，不请求授权。

### 4.2 RED-2：环境瞬态

Docker、依赖命令、锁或超时最多做三次有原因的重试。三次后标记 `ENVIRONMENT_BLOCKED`，冻结依赖 lane，继续静态审计、文档、其它测试和不依赖工作。

### 4.3 RED-3：硬停机但只冻结 lane

以下情况禁止猜测：

- fixed HEAD 或合同 SHA 漂移；
- 出现本任务外工作树改动；
- 磁盘不足 5GB；
- 目标 DB 不符合隔离库白名单；
- 需要正式库/生产写入；
- 权威来源冲突；
- 至少两个合理方案会改变业务事实、权限、持久化或不可逆迁移；
- 需要破坏性动作但没有授权；
- blocker 无法从来源唯一关闭。

处理：登记 `DECISION_REQUIRED` 或 `SOURCE_INCOMPLETE`，冻结受影响 lane，继续所有独立 Phase。不得中途把问题抛给用户。最终统一报告。

### 4.4 DECISION_REQUIRED 卡格式

只有在真实源码/schema/数据库/权威资料都核对后，仍存在两个以上会改变业务事实或安全边界的合理方案，才可创建：

```text
ID
阻塞 Phase/lane
精确文件/行
已查证事实
无法唯一推出的问题
方案 A / B
各自业务收益
各自风险
推荐及理由
未决期间已完成工作
受阻工作
可证伪验收
```

普通 bug、测试设计、私有函数组织不得包装成决策卡。

---

## 5. 上下文压缩与自恢复

handoff 初始取件路径固定：

`/private/tmp/V2-XHS-CONTENT-RELEASE-CANDIDATE-HANDOFF.md`

每个 checkpoint 必须先完整写入同目录临时文件，校验 `schemaVersion`、`BEGIN_HANDOFF`、`END_HANDOFF` 后原子 rename 覆盖，禁止 append 或原地半写。每次包含：

- contract SHA-256；
- `BASELINE_HEAD`、`EXPECTED_CHECKPOINT_HEAD`、提交 manifest、两仓 status；开始时 current 必须等于 baseline；之后 baseline 必须是 current 的祖先且 current 必须等于 handoff checkpoint，未登记 commit 必须 RED；
- 当前 Phase 和最后完成 test ID；
- 实际 diff；
- proof DB；
- 已完成门禁及 exit code；
- RED→GREEN；
- 冻结 lane / DECISION_REQUIRED；
- 下一条精确动作；
- 禁止事项遵守情况。

上下文压缩、重启或中断后，必须：

1. 重读两仓 `AGENTS.md`；
2. 重读本合同并验证 SHA；
3. 读取 handoff；
4. 检查两仓 baseline/checkpoint HEAD、提交 manifest、status/diff；
5. 检查 proof DB；
6. 重跑最后 checkpoint 的最小真实性门禁；
7. 从“下一条精确动作”继续。

Phase 0 必须做一次恢复演练：创建合法 checkpoint 后，新上下文不读聊天，仅凭合同、expected-SHA sidecar 与 handoff 恢复应 GREEN；篡改合同/handoff 任一字节或插入未登记 commit 必须 RED。

禁止依赖聊天记忆或“我记得已经通过”。

---

## 6. Phase 0：固定输入、安全和合同门禁

### 输入

- 本合同；
- §1 两仓固定点；
- A-04 审计文件及 hash；
- Docker/PostgreSQL；
- 至少 5GB 空闲磁盘。

### 必须执行

```bash
cd /Users/gongyong/Services/content-workbench/v2-b3-projection-readiness
git status --short --branch
git rev-parse HEAD
shasum -a 256 docs/code-review/v2-b3-caller-read-cutover-source-audit-2026-08-12.md
df -h /System/Volumes/Data /
docker inspect -f '{{.State.Status}} {{.State.Running}}' topic-dashboard-local-postgres

cd /Users/gongyong/Services/linggan-boom
git status --short --branch
git rev-parse HEAD
```

再计算本合同 SHA-256，与主线交付值精确相等。

### 持久恢复副本与插件隔离 worktree

初次校验后立即：

1. 使用 `git rev-parse --git-path codex-run-state/v2-content-rc` 取得 Git 私有运行态目录；禁止在工作树创建未忽略的 run-state。将本合同逐字节复制为 `master-contract.md`，同时写 expected SHA 只读 sidecar；两者 hash 必须与主线交付值相等。后续恢复以该副本为准，任何字节漂移 RED。
2. 将 handoff 从 `/private/tmp` 初始化副本迁移为同一 Git 私有目录的 `handoff.md`，以后通过临时文件 + 原子 rename 覆盖；不得 append。创建/更新全部运行态文件后 `git status --porcelain` 必须与创建前完全一致且恢复仍可读。最终原子写回 `/private/tmp/...HANDOFF.md` 供主线取件。
3. 原插件 `main` 禁止修改。若专用路径/branch 不存在，从固定 SHA 创建专用 branch/worktree（建议路径 `/Users/gongyong/Services/linggan-boom-v2-content-rc`、branch `v2/xhs-content-rc`）；若已存在则必须精确验证 HEAD/status/ownership，不得覆盖。把真实路径写入 `PLUGIN_EXECUTION_WORKTREE` manifest；所有插件修改/commit/test/build/package/cross-repo verify 只使用该路径，禁止硬编码或回退原 main。
4. 记录原插件 main 的 HEAD、tracked blob tree、index/status；任务结束必须一致。共享 Git common-dir 只允许新增 manifest 登记的 worktree 元数据；合同固定在主线审核前保留该 worktree，不得把预期 worktree metadata 误报为 main 漂移。handoff 分别报告原仓和执行 worktree。

### 攻击/自证

- 错一位 SHA 必须 RED；
- 多一个未知文件必须 RED；
- 审计文件 hash 漂移必须 RED；
- Docker 不 running 或磁盘 <5GB 时 DB lane 冻结；
- 禁止自动删除或覆盖未知文件来“恢复 clean”。
- 合法 checkpoint commit 后：baseline 仍是祖先、current 等于 handoff checkpoint 才 GREEN；current 不再要求永远等于 baseline。插入未登记 commit 必须 RED。

### 退出

仅当输入完全匹配，才可把 A-04 审计和 DR-B3-CALLER-001=A 的决策同步作为第一个本地文档 checkpoint。必须更新 `01-decisions.md`、`TODO.md`、必要的 progress/审计索引；不得把仍 OPEN blocker 改成 CLOSED。

---

## 7. Phase 1：来源台账与 Release Candidate 范围冻结

### 目标

形成当前固定点的可执行差距矩阵，不写 runtime。

### 必须核对

- `BLK-001/002/004-015` 当前均仍 OPEN；`BLK-003/016` CLOSED。
- Content 子集虽已有 55 项证明，不自动关闭 Author/Presentation 的全域 blocker。
- `BLK-015` 与 `BLK-010` 是 caller 前硬门。
- A-04 的 175 个文件是词法库存；只有已证实调用链可进入 C1。
- K-01 Material 是首个 Content-only 候选，但当前仍 0 READY。

### 输出

- blocker/decision/source 矩阵；
- 计划新增/修改文件清单；
- migration 顺序图；
- V1 写/读阻断清单；
- 每 Phase test ID manifest；
- Release Candidate 与上线操作的明确分界。

### 禁止

- 用本合同替代尚缺的字段/权限来源；
- 为了推进把 OPEN 改 CLOSED；
- 把全部 175 个词法命中都当产品消费者；
- 同步整改泛化文档或无关治理债。

---

## 8. Phase 2：BLK-015 Evidence Security Foundation

### 目标

在 runtime caller 之前建立数据库强制的唯一 Evidence 写入和受控读取/审计边界，同时收口 BLK-001/002 与该边界直接相关的权限、键和删除语义。

### 来源审计先行

逐字段/关系核对：

- `EvidenceAccessAudit`；
- `EvidenceReaderWorkspaceGrant`；
- CapturePackage/RawSnapshot/RawRecord 的同 workspace keys；
- `evidence_writer`、`canonical_writer`、`default_app`、schema owner；
- `SECURITY DEFINER` 受控读取函数；
- `session_user` 与 `current_user`；
- GRANT/REVOKE；
- append-only；
- audit 与真实 CapturePackage 的同 workspace 关系；
- restricted 读取；
- reader grant 创建、撤销和检查 owner。

缺少首切所需 `evidence_writer`、reader、`canonical_writer`、`default_app`、function owner 任一 runtime 身份/DSN/EXECUTE/GRANT 时，Phase 2 必须 BLOCKED。只有由静态 caller manifest 证明 Content 首切不可达的可选 grant 管理 UI 可以单独后置。

### 实施边界

- 所有角色/GRANT/REVOKE/SECURITY DEFINER 测试必须使用独立一次性 PostgreSQL container/cluster、独立端口和独立数据目录；严禁在承载 `content_workbench_local` 的 `127.0.0.1:54329` 实例创建、修改或删除角色/ACL；
- 测试前后只读导出正式实例 `pg_roles`、database ACL 与 `content_workbench_local` schema/data 指纹，必须完全一致；
- handoff 固定独立 cluster 的容器 ID、image/PG major、host、port、server fingerprint、数据库/角色 manifest；任何连接到 54329 的角色测试必须在 DDL 前失败；
- pure expand migration 可先落地候选；
- 最终 REVOKE/角色切换写入单独 cutover migration，不在正式库执行；
- 生产 `EvidenceArtifactAuditSource` 必须通过受控函数读取真实 package bytes 并落审计；
- B2/B3 不得默认直接查询 CapturePackage payload；
- 不提供绕过审计的 default reader；
- UPDATE/DELETE Evidence/Audit 必须数据库拒绝。

### 生产运行时身份必须真实分离

- `evidence_writer`、受控 Evidence reader、`canonical_writer`、`default_app` 必须使用独立 DSN/连接工厂和最小权限登录角色；schema/function owner 必须是专用 `NOLOGIN`；
- 禁止所有运行时代码回退到通用 `DATABASE_URL`，禁止用 `SET ROLE` 模拟身份；缺失/错配 DSN 必须启动失败；
- 函数外必须断言 `current_user=session_user=runtime login`；SECURITY DEFINER 函数内必须断言 `session_user=authorized reader login`、`current_user=exact NOLOGIN function owner`，调用返回后恢复 login identity；其它组合、role membership、SET ROLE 全部失败并记录 owner/ACL；
- 静态扫描所有 V2 runtime client 创建点，证明无 owner/admin/default client 绕过；
- 隔离库用每个真实 runtime client 分别证明合法动作成功和越权 SELECT/INSERT/UPDATE/DELETE 精确 SQLSTATE 拒绝。

### SECURITY DEFINER 硬化

- 固定空或仅可信 schema 的 `search_path`，所有对象全限定名；
- owner 为不可登录专用角色；
- `REVOKE ALL ON FUNCTION ... FROM PUBLIC`，只给精确 reader `EXECUTE`；
- 恶意同名 schema/table/function、PUBLIC/其它角色调用、ALTER search_path、直接写 audit、function owner 登录均必须失败；
- 权威角色逐一正/负测试，不只测试 default_app。

### PostgreSQL 攻击测试

至少固定以下 ID：

- `SEC-01` 合法 reader + workspace grant 返回真实 bytes 且写真实 audit；
- `SEC-02` default_app 直读 package payload 被拒绝；
- `SEC-03` default_app 直写 RawSnapshot/RawRecord 被拒绝；
- `SEC-04` 非 evidence_writer 写 Evidence 被拒绝；
- `SEC-05` 跨 workspace grant/audit/package 被拒绝；
- `SEC-06` 伪造 audit 后读包被拒绝；
- `SEC-07` audit UPDATE/DELETE SQLSTATE 固定拒绝；
- `SEC-08` Evidence UPDATE/DELETE SQLSTATE 固定拒绝；
- `SEC-09` revoked grant 不能读；
- `SEC-10` restricted 包无授权不能读；
- `SEC-11` 伪 GUC / `SET ROLE` / session_user 冒用不能绕过；
- `SEC-12` reader 失败不留孤立成功 audit；
- `SEC-13` B2 合法受控读取不回归；
- `SEC-14` V1 execution 现役写入在硬切前候选 schema 上不回归。
- `SEC-15` 每个 runtime DSN 的 current_user/session_user 与权限矩阵精确一致；
- `SEC-16` 错/缺 DSN、通用 DATABASE_URL fallback、SET ROLE 全部启动或事务前失败；
- `SEC-17` SECURITY DEFINER search_path/PUBLIC/owner 攻击全部失败；
- `SEC-18` 正式 54329 实例只允许在独立 cluster 测试前后做只读 `pg_roles`/database ACL/schema/data fingerprint 导出且结果一致；任何测试 DDL/DML/CREATE ROLE 连接 54329 必须在执行前失败。

### 成功定义

不是“函数被调用”，而是：公开受控 reader 返回与 package checksum 一致的真实 bytes，同时持久化同 workspace、同 package、同 principal 的 append-only audit；所有绕过路径由 PostgreSQL 拒绝。

---

## 9. Phase 3：durable V2 后台推进与 TaskStatus 五轴

### 已确认业务方向

DR-B3-CALLER-001=A。Evidence 成功后后台推进；插件请求不等待 Projection；任务成功不能代替 B2/B3 成功。

### 必须先来源化的物理缺口

现有 EvidenceIngress 事务只写 CapturePackage、Receipt、Artifact、RawSnapshot、RawRecord。现有资料没有自动授权：

- 新建 V2 Work/Outbox 表；
- 复用 V1 `OutboxEvent`；
- Receipt polling；
- Evidence 提交后 best-effort enqueue。

绝对禁止 best-effort enqueue，因为 Evidence 已提交而事件丢失会永久不推进。

执行代理必须审计现有 outbox schema、transaction seam、07 固定事务和 TaskStatusProjection 来源。若无法唯一推出 durable handoff 的物理 owner，创建一张完整 `DECISION_REQUIRED`，冻结整个 durable 实现 lane；只允许继续来源审计、决策卡、行为级验收矩阵、故障案例和不绑定生产 interface 的固定 fixture。冻结 lane 在 `src/`、schema、migration 中必须零新增；禁止针对想象 seam 新建 interface、validator、worker 或测试。其它不依赖 lane 继续，最终关键 lane 未解锁只能 `BLOCKED`。

### 若来源可唯一推出，实施必须满足

- Evidence 与 durable work 在同一可证明提交边界，或由数据库可证明无丢失的 receipt claim 机制承接；
- payload 只存 identity/version，不复制 Evidence 事实；
- B2 claim/每次重试从 DB 重验 workspace、receipt、snapshot、integrity、contract/version，不要求尚不存在的 CEC；B2 提交成功后，B3 claim/每次重试才重验 current CEC、accepted、Input 与绑定 Observation；
- B2 与 B3 保持各自事务；
- retry/dead/attempt/lease 有唯一 owner；
- 同 work replay 幂等；
- 双 worker 不能重复推进；
- 40001/40P01 有界重试；
- dead-letter 可审计且重放仍重验事实；
- TaskStatusProjection 五轴只由已确认 writer 推进。

### 攻击测试

- `WRK-01` Evidence 后/B2 前 worker crash 可恢复；
- `WRK-02` B2 后/B3 前 crash 可恢复；
- `WRK-03` B3/Media 中间失败零部分 Projection/Usage；
- `WRK-04` enqueue/persist 返回 `{success:false}` 整体不得报成功；
- `WRK-05` 双独立 client 并发同 work 收敛；
- `WRK-06` duplicate/stale event 不推进错误 Current；
- `WRK-07` 40001/40P01 真实重试；
- `WRK-08` dead-letter replay；
- `WRK-09` payload workspace/snapshot/evaluation 篡改被 runtime validator 拒绝；
- `WRK-10` task succeeded 但 B2 pending 时 Projection 仍不可见；
- `WRK-11` B3 success 由公开 read seam 和 DB 状态共同证明；
- `WRK-12` 无 work 丢失分母证明。

### TaskStatus 五轴

不得凭字段名猜值域。BLK-010 五轴只能是既有 `executionStatus` 加新增 `evidenceStatus`、`contractStatus`、`projectionStatus`、`mediaStatus`；禁止增设/改名为 collection/normalization/canonical/presentation 等不存在字段。逐轴核对值域、终态、唯一 writer 和迁移顺序；来源不足则冻结 TaskStatus lane并保持 blocker，不伪造 `completed`。schema/validator/SQL/test 全仓对错误字段名必须零命中。

---

## 10. Phase 4：Release-B ingress 与插件硬切候选

### 目标

生成 execution/manual_import/recovery 到唯一 EvidenceIngress 的候选代码与插件包；migration 仍无 caller。候选代码内旧的新数据路径不可达，不做 feature fallback。

Release Candidate 必须产出并 pin 两个不同、可复现的工作台 artifact/SHA：

1. `pre-cut dark artifact`：V1 入口仍是唯一生产入口，V2 security/worker/read 代码存在但 worker disabled、无 V2 caller；只允许 D2 部署。
2. `atomic hard-cut artifact`：execution/manual/recovery 只进 V2，V1 新写/旧协议不可达；只允许维护冻结后的 D4 部署。

两者必须有独立静态扫描、build fingerprint 和 manifest，禁止用同一个 artifact 通过运行时 flag 在 V1/V2 间选路。

### 双 artifact 的不可变提交图

两个 artifact 必须对应两个真实、可 checkout 的工作台提交，且提交关系固定为 `A → B`：

- `A = PRE_CUT_SHA`：包含 Evidence security expand、disabled durable worker、暗态 read service，以及最终版本的 V2 deploy/preflight/observe/freeze 集成、脚本和测试；生产 V2 ingress caller、hard-cut migration、Material read switch、V2 worker enable 在 A 中必须零命中。
- `B = HARD_CUT_SHA`：必须是 A 的严格后代；只在 A 基础上加入 final hard-cut migration、execution/manual_import/recovery 三个 caller、Material 单读切换和 V2 worker enable，不得删除或绕过 A 的安全与部署门禁。

必须证明 `git merge-base A B` 精确等于 A；分别 checkout A/B，实际执行 build、专属静态扫描和 migration manifest 对账。D2 只允许部署 A，D4 只允许部署 B。插件候选使用独立 `PLUGIN_SHA`/包 hash 固定，不得把插件版本隐含在工作台提交图中。任何最终修复若改变 A 的内容，必须重建并重新固定 A，再从新 A 重建 B，两个候选的全部证明一并失效并重跑；禁止只移动标签或复用旧报告。

### execution

- 从真实 terminal DeltaOutbox 链进入签名 V2 envelope；
- 严格 session capability 绑定 method/path/body hash；
- `sourcePrincipal=execution-station:<stationId>`；
- station/authorization/job workspace 严格相等；
- lease 控制事实与 Evidence 事实分离；
- Evidence 已提交但控制 CAS 失败时可审计重入，不重复 Evidence；
- 旧 `commit_raw_snapshot` 不再承接新包。

### manual_import

- 当前 V1 会创建 Job/Runtime；V2 路径必须移除该伪执行关系；
- authority 从已验证用户/authorization 注入；
- `sourcePrincipal=user:<userId>`；
- 原子切到 EvidenceIngress；
- 不再直写 Topic/Comment/Author/Media 业务表。

### recovery

- owner/admin；
- `sourcePrincipal` 与 `recoveryAuthorizedBy` 都来自会话；
- 不创建/推进 Job/Attempt/Queue/Runtime；
- 不借既有 jobId 伪装 V2 来源。

### migration

- 首期无 API/CLI/cron/runtime caller；
- 只保留合同与测试；
- 既有 migration authority DECISION_REQUIRED 不阻塞 Content 首切。

### 插件

- 六份真实、脱敏、固定 fixture；
- 插件与工作台独立 canonical/hash；
- 跨仓 6/6；
- 任一 payload/header/hash/contract 变化必须自动失败；
- 生成可安装包、SHA-256、版本与九工位升级清单；
- 不得宣称九工位已升级或实测。

提供受测试、无副作用的本地打包入口（如 `package:candidate --output <allowlisted-temp-path>`）：不得修改 version/manifest/源码，不得 git add/commit/push/publish；before/after tracked hash 与 status 精确一致，仅允许白名单 zip 新增，zip 文件清单/hash 与 dist 一致。禁止调用会产生 Git/发布副作用的 `release:*` 或 `scripts/version.sh`。

### 攻击测试

- `ING-01..04` 四 authority 正例；
- `ING-05` 非执行来源出现 job/attempt/station 拒绝；
- `ING-06` identity+同 hash replay；
- `ING-07` identity+异 hash 保留 conflict、不投影；
- `ING-08` package/header/checksum/length 篡改；
- `ING-09` 跨 workspace；
- `ING-10` 伪 capability/另一 body/replay session；
- `ING-11` Evidence committed + control CAS failure 重入；
- `ING-12` V1 `commit_raw_snapshot` 对新协议不可达；
- `ING-13` 旧 raw/business direct writers 静态扫描；
- `ING-14` 六 workflow 真实 fixture；
- `ING-15` 插件本地 outbox 暂时/永久失败和 lease 清理；
- `ING-16` migration caller 零命中。

---

## 11. Phase 5：全链暗态覆盖与事实完整性

### 公开 seam

真实 fixture 必须从公开 ingress/worker/read seam 进入，不得通过私有 writer 直接造成功。

### 链路

```text
CaptureSubmission
→ EvidenceIngress
→ durable handoff
→ controlled Evidence Reader/Audit
→ B2
→ B3 Content + CanonicalMediaSlot
→ Media Domain + ContentMediaUsage
→ Content Projection DTO
```

### 必须证明

- replay/conflict；
- ARCHIVED 可按合同重算；
- REDACTED/PURGED 拒绝；
- accepted A→B 先隔离再恢复；
- rejected 可以推进 `ContractEvaluationCurrent`，同时必须 quarantine 既有 Content Current、关闭 active Usage；不得创建/推进新的 ContentObservation、`currentObservationId` 或 projectionVersion；
- 同 Observation/同 Evaluation exact replay；
- 同 Observation/新 Evaluation 重建 Usage；
- 一个 MediaItem 多个 Canonical slot；
- late slot 不破坏 visible completeness；
- public Content Projector 只接受恰好一个 note 的输入；多 note package 可保留 B2/CEC 事实，但不得产生、推进或撤销任何 Content Projection/Usage，禁止 `LIMIT 1` 或逐 subject 分流；用含两个不同 subject 的 accepted 包证明 Domain mutation 为 0；
- ambiguous identity 全事务失败；
- `live_photo` fail closed；
- 缺 candidate 不生成 absent/unavailable；
- 更新/删除不可变事实拒绝；
- 所有跨 workspace 关系拒绝；
- fault injection 后相关表零残留；
- success 可从最终 read seam 观察。

---

## 12. Phase 6：唯一 Content Projection Read Service

### 输入

只接受可运行时验证的：

- workspaceId；
- platform=`xhs`；
- platformContentId 或已证明的稳定 Content identity。

### 唯一读取

一个数据库一致性快照中读取：

- visible ContentCurrentProjection；
- 它指向的 ContentObservation；
- current accepted ContractEvaluation/Input；
- active V2 ContentMediaUsage 六项 provenance；
- Media Domain 的可交付副本；
- 同一接受观察的 originalUrl。

### DTO

版本化 Content-only DTO 至少包含：

- stable identity；
- projection revision/version；
- title/body/type/publishedAt；
- originalUrl；
- cover；
- ordered observed media；
- explicit unavailable/null，而不是猜值。

### 禁止

- 读 RawSnapshot/RawRecord/CapturePackage；
- 用 V1 ContentAsset 展示字段补值；
- 直接拼平台 URL；
- 页面自己查 Media；
- `V2 ?? V1`；
- 字段级混合不同 revision；
- 用 `as` 替代 runtime validation。

### 攻击测试

- `READ-01` current accepted visible 正例；
- `READ-02` rejected/quarantined 不可见；
- `READ-03` 跨 workspace；
- `READ-04` Current 与 Observation/CEC 错绑；
- `READ-05` active Usage provenance 缺一项；
- `READ-06` V1 与 V2 sentinel 不同，只返回 V2；
- `READ-07` V2 缺失时明确 unavailable，不返回 V1；
- `READ-08` 并发 supersede 不见混合 revision；
- `READ-09` invalid originalUrl；
- `READ-10` cover 顺序规则；
- `READ-11` late slot/Usage completeness；
- `READ-12` Raw/Media 直读静态扫描。

---

## 13. Phase 7：Material Content-only 单读硬切候选

仅允许修改 A-04 的 K-01 完整调用链；其它 Topic/Monitor/Radar/Author/Comment/Metric/AI 消费者不得夹带。

### 规则

- Material 页面/API 只接 Phase 6 DTO；
- 一次切全部 Content-only 字段，禁止只切 URL 形成混合 revision；
- V2 不可用时页面显示明确不可用/暂不可展示；
- 不回退 V1；
- 不在同一阶段物理删除旧表/字段；
- 旧模块可暂存于 repo，但新 consumer 必须静态不可达。

### E2E

- `CUT-01` V1/V2 注入不同 title/body/url/cover，页面只展示 V2；
- `CUT-02` V2 缺失，页面不展示 V1；
- `CUT-03` rejected/quarantined；
- `CUT-04` workspace；
- `CUT-05` revision 并发；
- `CUT-06` 媒体缺失；
- `CUT-07` invalid originalUrl；
- `CUT-08` 页面/API/service 无旧字段 fallback；
- `CUT-09` 真实浏览器或公开 route 行为，不只测 serializer；
- `CUT-10` Author/Comment/Metric 未被误切。

用户体验阈值、暗态覆盖率和真实硬切时点若仍未确认，代码保持 release candidate，不宣称上线授权。

---

## 14. Phase 8：隔离数据库总证明

### 安全门禁

- 所有本 Phase 数据库都位于 Phase 2 创建的独立一次性 PostgreSQL cluster；禁止 host/port `127.0.0.1:54329`；
- opt-in 必须精确等于 `V2_CONTENT_RC_INTEGRATION_DB=1`；
- 新增总证明 DB 名必须匹配 `content_workbench_v2_content_rc_proof_<timestamp>`；旧套件必须先统一重构为接受同一经验证独立 cluster，或建立逐套件数据库前缀白名单 manifest；任何未列前缀/数据库、54329、远程 host 必须在建库前失败；
- 首次任何 schema/数据写入前连接并断言 `current_database()`；
- 明确拒绝 `content_workbench_local`、所有未匹配名字和远程 host；
- 用固定点 schema + 本次全部候选 migration 在全新库构建；
- 禁止 `--accept-data-loss`；
- proof DB 不自动删除；
- 打印精确 DB 名和人工删除命令。

### Prisma 特别规则

本仓 `prisma.config.ts` 会加载 `.env.local`。不能用 `DATABASE_URL=...` 的表面赋值推断目标，也禁止临时修改 `.env.local`。必须先实现唯一允许的隔离 DB harness：独立 cluster 的 host/port/db/user 以显式参数传入 Prisma/psql/client；执行前后 `.env.local` hash 不变；捕获 Prisma datasource 并与 `current_database()`、`inet_server_port()`、server fingerprint 三重匹配。任何 datasource 指向 54329、`content_workbench_local` 或非 manifest DB，必须在 schema/migration 写入前非零退出。

### helper 自证 ID

- `H-01` proof 验收命令缺 opt-in 必须非零退出，禁止 skip；普通全量测试可以跳过隔离套件但不得计入 proof gate；
- `H-02` 错 DB 名在首次写入前失败；
- `H-03` 正式测试库/生产 URL 拒绝；
- `H-04` current_database 不匹配失败；
- `H-05` 带正确 opt-in 的 proof runner executed test IDs >0，missing/duplicate/unexpected/skip/todo 全为 0；
- `H-06` fault hook 确实执行；
- `H-07` fault 后所有相关表计数不变；
- `H-08` 全新库、无历史依赖；
- `H-09` proof DB 保留；
- `H-10` V1/V2 sentinel 可证伪 fallback；
- `H-11` SQLSTATE/领域错误精确匹配；
- `H-12` 并发用两个独立连接；
- `H-13` expected hash 来自独立 fixture/固定字面量，不调用被测函数生成；
- `H-14` downstream `{success:false}` 必须让链路失败；
- `H-15` 负向 SQL helper 用成功 SQL 自证不会假通过；
- `H-16` expected/executed/missing/duplicate/unexpected 五项 manifest 对账。

必须分别运行缺 opt-in 与正确 opt-in 两次，前者真实 RED、后者才可 GREEN。最终数据库 manifest 的 expected 与独立 cluster 实际新建数据库集合必须完全一致。

### 最终数据库不变量

- Evidence 五表/安全模型/worker/Derived/Canonical/Projection/Media 各表计数；
- 跨 workspace 零错绑；
- conflict 零 visible；
- rejected 可以推进 ContractEvaluationCurrent；Content `currentObservationId`/projectionVersion 必须零推进，既有 Content Current 必须 quarantined，active V2 Usage 必须为 0；四项分别断言；
- visible Current 的 observed slots 全部有唯一 active Usage；
- V2 worker work 无丢失分母；
- V1 direct new writes 在候选 hard-cut 状态不可达；
- 所有测试 wrapper、barrier、sequence、临时函数零残留。

### 现有套件最低复跑清单

下列命令是当前固定点已有证明入口，不代表覆盖本次新增代码；必须复跑，并另增“当前全部候选 migration 从零应用”的总套件：

```bash
V2_B1_INTEGRATION_DB=1 npx vitest run src/lib/evidence/ingress/evidence-ingress.integration.test.ts
V2_B2_INTEGRATION_DB=1 npx vitest run src/lib/evidence/derived/b2-derived-schema.integration.test.ts
V2_B2_SERVICE_INTEGRATION_DB=1 npx vitest run src/lib/evidence/derived/b2-derived-service.integration.test.ts
B3_MEDIA_INTEGRATION_DB=1 npx vitest run src/lib/evidence/media/canonical-media-slot.integration.test.ts
B3_MEDIA_DOMAIN_INTEGRATION_DB=1 npx vitest run src/lib/evidence/media/canonical-media-domain.integration.test.ts
B3_PROJECTION_INTEGRATION_DB=1 npx vitest run src/lib/evidence/projection/content-projection.integration.test.ts
```

若本次修改 migration 后这些测试仍只使用旧固定 schema，它们只能算回归，不能算候选 migration 证明。

---

## 14A. Phase 8A：V2 可观测、预检与静态单轨

### 为什么必须单列

当前实现中，多项旧链/绕过/五轴事件尚无探针。查询结果为 0 可能只是“没有记录”，不能证明“没有发生”。现有部署 healthcheck 只验证 auth session；现有 media preflight 也不是 V2 总门禁。

### 必须实现的版本化事实

- Evidence receipt committed/conflict/rejected；
- durable worker pending/running/retry/dead/oldest age/attempt；
- B2/B3 每层状态与 latency；
- 旧协议拒绝；
- V1 新写尝试；
- 页面旧读、Evidence 直读、Media 直查和 fallback 尝试；
- 跨 workspace/权限拒绝；
- 九工位版本/协议水位输入（候选只提供采集接口，不伪造九机数据）。

### 命令/脚本候选

提供机器可读、数据库权限强制只读、exit-code fail-closed 的：

- `v2:cutover-preflight --mode pre_cutover`：输出 fixed SHA、probe version、workspace、planned window、备份/冻结/队列/九机/migration/artifact 各门禁；切前不要求 V2 生产分母，任何 pre-cut V2 新写反而失败；
- `v2:cutover-preflight --mode post_cut_canary_ready`：只在 D4 硬切完成、D5 canary 尚未提交的窗口运行；从权威部署/数据库事实验证不可变 actual cutoverAt、HARD_CUT_SHA、角色/协议、V2 worker enabled+version+heartbeat+claim readiness、V1 worker stopped、维护冻结仍生效，并断言业务分母此时预期为 0；任一不符 exit 1；
- `v2:observe --mode post_cut_observe`：actual cutoverAt 必须来自硬切部署/事务记录，输出真实非空分母、pending/retry/dead/latency/old-chain/bypass/workspace；
- `v2:single-track-static-scan`：扫描生产源码/schema/migration/package scripts。

命名可按仓库规范调整，但不得复用 media-only scan 冒充 V2 全链。

### 只读身份与预检硬规则

- preflight/observe 使用独立 read-only 登录 DSN，`default_transaction_read_only=on`，无 DML/DDL/sequence/side-effect function 权限；必须拒绝 admin/evidence_writer/canonical_writer/default_app DSN；隔离库恶意写 probe 必须 SQLSTATE 25006/权限拒绝且关键表 before/after 指纹一致；
- 三种模式都先断言 `current_database()`、host、workspace；两个 post 模式另断言不可变 actual cutoverAt；
- `pre_cutover` 必须检查：fresh custom dump + `pg_restore --list`/独立恢复证据、磁盘、migration checksum/ledger、pre-cut/hard-cut artifact 与插件 SHA、持续维护冻结、active job/lease/旧 outbox=0、九机真实 probe=9/9；不检查 post-cut 覆盖率；
- `post_cut_canary_ready` 必须检查 hard-cut 部署和 worker/角色/协议已就绪、V1 worker 已停、intake 仍冻结，并要求业务分母恰为 0；它不得把空分母当异常，也不得提前产生 canary 事实；
- `post_cut_observe` 分母必须非空并逐层对账；
- probe version/SHA 必须匹配候选；
- 缺表、缺探针、post 空分母、旧链 >0、dead >0、跨 workspace >0、未对账 work >0 都必须 `ready=false` 且 exit 1；
- 不得 catch 后输出 ready；
- 事件表/状态写入必须 runtime validate，并有伪造/UPDATE/DELETE/跨 workspace 攻击测试。

每个门禁必须绑定权威 reader，禁止 `--*-ok`、环境布尔值、mock/人工 JSON 作为成功事实：backup 来自实际 custom dump hash、`pg_restore --list` 和恢复库指纹；freeze/drain 来自 server-side maintenance state 与 lease/job/queue/outbox 查询；九工位来自服务端 station registry 的实际版本/协议/lastSeen/account probe；artifact 来自实际 deploy commit/ref、lockfile、build fingerprint 与插件 zip manifest；migration 来自目标库 ledger；cutoverAt 来自 hard-cut 部署/事务不可变记录。每项输出 `observedAt/freshness/sourceIdentity/sourceHash`；篡改、过期、缺源、另一 DB/部署 SHA 均 exit 1。

Phase 8A 的 `CHECKPOINT_COMPLETE` 只要求命令/探针实现和独立环境正负演练。真实生产 backup/freeze/9机 pre-cut gate 必须在 RC handoff 标 `NOT_EXECUTED_OPERATOR_GATE`：它不阻塞本地 `READY_FOR_OPERATOR_CUTOVER`，也不得写 GREEN；只能由主线审核后的 D1/D3 执行。

hard-cut 前必须在独立候选环境对每个旧读/旧写/旧协议/bypass probe 做受控正向触发，证明计数从 0→1；然后重建干净 proof DB。生产输出携带 probe manifest SHA/version、部署 SHA、actual cutoverAt、heartbeat/lastSeen；heartbeat 缺失时状态是 `UNKNOWN` 且 exit 1，绝不能当 0。

### 实际生产部署入口必须纳入候选

不能只新增无人调用的 V2 脚本。必须修改并攻击性测试现役 `.github/workflows/deploy-mac-mini-production.yml` 与 `scripts/ops/deploy-mac-mini-production.sh`（或全仓证明的唯一替代入口）：明确 `pre_cut_dark(D2) → pre_cutover(D3末) → hard_cut(D4) → post_cut_canary_ready → canary(D5) → post_cut_observe` stage；pin 双 commit/ref、lockfile、build fingerprint、插件 SHA；现役入口按 Git ref checkout+build 部署，因此 artifact 默认指可实际部署 manifest；按阶段调用对应 V2 preflight/observe，media-only gate 不得冒充；freeze token/state 跨 restart 持续且 D6 前任何 branch 不得 unset；同一入口 fail-closed drain 所有 intake/runner/scheduler/outbox；migration 后失败保持 freeze、停止 V2 worker并 fix-forward。测试固定命令顺序并攻击错 stage/ref、旧 gate、缺 freeze、重启、health failure、重复执行、错误 worker 状态，以及跳过/乱序/复用过期结果；任一 artifact SHA、插件 SHA、目标 DB、freeze token、backup/restore、drain 或九机水位漂移都必须阻止进入下一 stage。未通过则 Phase 8A BLOCKED。

### 静态单轨最低扫描

候选 hard-cut 状态必须证明生产可达代码中：

- V1 RawSnapshot/RawRecord 新 writer = 0；
- 旧 `raw_snapshot.committed` producer = 0；
- 旧 Evidence/Raw 直读 = 0；
- ContentAsset 展示字段读取（K-01 路径）= 0；
- 页面直接查 Evidence/Media = 0；
- V1/V2 fallback/feature flag 选路 = 0；
- 新协议到 V1 handler = 0。

历史、测试和后置模块豁免必须逐文件固定清单 + hash，不得宽泛豁免整个目录。

### 并行验证的唯一合法形式

AGENTS 的“先并行验证再删旧路径”在本 V2 任务中仅指：隔离库、固定水位离线回放和只读覆盖率审计。禁止一个产品请求同时读取 V1/V2，也禁止一个新提交写两条链。切读和物理删旧可分阶段，但切读后旧读取必须不可达且无 fallback。

---

## 14B. Phase 8B：全 migration 顺序与 hard-cut 预演

### 必须证明

- 全新隔离库按真实 migration 顺序从零成功；
- 固定点 schema/fixture 到 expand 再到 hard-cut candidate 成功；
- 每个 migration checksum 固定；
- 第二次 `prisma migrate deploy` 由 `_prisma_migrations` 识别为 no-op；不要求直接重复执行原 SQL 文件；
- final hard-cut migration 必须显式单事务；若存在无法事务化 DDL，必须拆成有 checkpoint 的可恢复 migration，固定可见部分状态、resume/fix-forward 命令和人工 abort 门；
- 对每个可注入失败点证明 catalog/权限/关键计数回到 before；不可事务化步骤按上条固定恢复语义；
- 切前 V1 execution 不回归；
- hard-cut 后 V1 协议/直写被拒绝；
- append-only、角色、受控 reader、worker/read DTO 均与 Prisma schema 一致；
- 不依赖 `.env.local` 覆盖后的错误 datasource。

### 备份/恢复演练（仅独立 cluster）

- 对 proof DB 生成 custom-format dump；
- 使用相同 PostgreSQL major 的 `pg_restore --list` 验证；
- 恢复到第二个全新白名单 proof DB；
- schema、migration ledger、关键表计数和内容指纹一致；
- 保留源 proof DB、恢复 DB 与 dump，写入 handoff；
- 绝不触碰正式测试库/生产库。

### 回滚边界

现有生产部署脚本在迁移前会把自动回滚禁用，迁移后的健康失败不能自动回滚数据库。因此 runbook 必须把 migration 后失败定义为：保持维护/停止 intake/停止领取/修复后向前重放同一 Evidence。不得自动切回 V1。

恢复 cutover 前数据库备份会丢失 post-cut V2 Evidence，只能作为人工灾难恢复决策，执行代理不得运行或预先承诺自动恢复。

---

## 15. Phase 9：全量质量门禁

在最新工作树、所有最后修改之后顺序执行并记录实际 exit/数量：

### 工作台

```bash
npx prisma validate
npx prisma generate
npx tsc --noEmit --incremental false
npm run test
npm run lint
npm run build
node scripts/check-project-governance.mjs --json
git diff --check
```

还必须运行仓库已有的：

- targeted Evidence/B2/B3/Media/Projection/worker/read/cutover tests；
- opt-in proof suite；
- cross-repo XHS contract verification；
- migration SQL 编译、顺序、重复应用/升级检查；
- single-track static scans；
- trace enforce/verify；
- build fingerprint write/verify；
- untracked 文件逐个 `git diff --no-index --check /dev/null <file>`；
- 正式库名、`--accept-data-loss`、unsafe production SQL、debug hook 静态扫描。

`git diff --no-index --check` 判读固定：新增文件正常有 diff 时 exit 1 且无 whitespace diagnostics 才通过；exit 0 表示无 diff；任何 whitespace diagnostics 或 exit >1 失败。另用 `git status --porcelain` 与 expected untracked manifest 对账，禁止用 no-index 命令证明工作树 clean。用一个干净新增文件和一个 trailing-whitespace 临时文件自证 helper 前者通过、后者失败。

当前跨仓入口至少包含（`$PLUGIN_EXECUTION_WORKTREE` 来自 Phase 0 已验证 manifest）：

```bash
npx tsx src/lib/evidence/contracts/cross-repo-verify.ts "$PLUGIN_EXECUTION_WORKTREE"
```

path guard 必须拒绝原 main、未登记路径和 SHA 不匹配 worktree。给原 main 与候选 fixture 注入不同 sentinel 的测试中，verifier 必须只反映候选值；传原 main 必须 RED。

### 插件

- 以下所有命令的工作目录都必须精确等于 `$PLUGIN_EXECUTION_WORKTREE`；
- `npm run check:contracts`；
- 全部插件测试，至少包含 `npm run test:douyin` 的现有回归（Douyin 仍不进入 V2 首切）；
- build；
- package/manifest/version/fixture/hash 校验；
- 生成本地候选安装包并计算 SHA-256；若 `release:*`/version 脚本会自动 git add/commit 或发布，禁止调用，改用无副作用构建入口；
- `npm run release:verify` 仅在已有本地 zip 且确认无发布副作用时运行；
- 确认无工作台 URL/生产凭据被打包；
- 不修改/发布远端版本。

### 治理

governance exit 1 不能写 0 issues。Phase 0 在固定 SHA 独立运行并 pin 结构化 baseline failures；当前已知三项是 Markdown 353 drift、TODO 229、`execution-sync-service.ts` 2022 行，但必须以真实输出为准。候选再次运行并机器比较：`candidate-new=0`、既有项未恶化且原样报告。满足时状态写 `BASELINE_DEBT_ACCEPTED`，不得写 exit0/GREEN；新增或恶化项必须修，否则 BLOCKED。禁止为 exit0 做泛化整改或修改检查器隐藏债务。

最后一个代码修改发生后，必须重跑受影响 directed tests 和全量门禁；旧运行数字失效。

---

## 16. Phase 10：执行代理自修与双轴 review

执行代理先基于固定点审查完整 diff，而不是只看测试：

### Spec 轴

- 每条已确认架构规则是否落实；
- 是否遗漏 Phase 验收；
- 是否出现范围蔓延；
- 是否把 SOURCE_INCOMPLETE 写成事实；
- 是否有部分成功/假成功；
- 是否把候选说成部署完成。

### Standards 轴

- repo coding/file/document standards；
- runtime validation；
- transaction/error/retry；
- Prisma/SQL 权限；
- 测试隔离；
- 文档真实值；
- Fowler smells 仅作判断项。

发现范围内 P0/P1/P2：代理自行补 RED、修复、重跑，不发 R1/R2 给主线。直到没有已知范围内 P0/P1/P2，或明确冻结 lane。

主线仍会独立重新审核，代理自审不能替代主线放行。

---

## 17. Phase 11：本地 Release Candidate 提交与包

### 提交策略

允许本地阶段小提交，禁止 push。每个提交必须单一职责、可审计、包含对应测试/文档。最终交付必须收敛为 §10 定义的不可变 `A → B` artifact 图：

1. 先完成 baseline/decision/docs、Evidence security expand、durable worker 暗态、read service 暗态，以及最终 deploy/preflight/observe/freeze 脚本、runbook 和测试；
2. 从上述最终内容固定 `A = PRE_CUT_SHA`，证明 A 中 hard-cut migration、三个生产 V2 caller、Material read switch、worker enable 均为零；
3. 以 A 为唯一父系创建后续提交，加入 ingress/plugin hard-cut candidate、final migration、Material cutover candidate和 worker enable；
4. 完成全部 proof/gates/review fixes 后固定 `B = HARD_CUT_SHA`；若修复触及 A 应有内容，必须重建 A，再从新 A 重建 B并重跑双候选门禁；
5. 分别 checkout A/B 重跑 build、静态扫描、migration manifest、lockfile/build fingerprint，并证明 `git merge-base A B == A`。

不得把未完成 lane 伪装进 commit message，也不得用工作树 patch 或运行时 flag 补足任一候选。最终工作树应 clean；若存在生成证明文件，必须明确归档或列为未提交产物。

### Release Candidate 产物

- 工作台最终本地 SHA；
- 插件最终本地 SHA；
- 两仓 commit 列表；
- migration 顺序和 checksum manifest；
- 插件安装包和 SHA；
- 九工位升级清单（未执行）；
- preflight/backup/restore/abort/fix-forward runbook；
- cutover migration/runbook（未执行）；
- 48h 观测 SQL/指标/报警/abort 条件；
- Release-C 后置清单；
- proof DB 名称；
- 全测试 manifest；
- 最终 handoff。

### COMPLETE 判定

只能使用：

- `READY_FOR_OPERATOR_CUTOVER`：全部 Release Candidate 关键 Phase 完成，等待主线审核和人工上线；
- `BLOCKED`：存在关键冻结 lane；
- `FAILED`：范围内无法恢复的工程失败。

禁止写“COMPLETE，但仍有以下未完成事项”。

关键 Phase 明确为：2、3 durable owner/worker、4、5、6、7、8、8A、8B、9、10、11。任一关键 Phase 未达 `CHECKPOINT_COMPLETE`，或相关 DECISION_REQUIRED/SOURCE_INCOMPLETE、proof skip/todo、未修复本轮治理失败存在时，只能 `BLOCKED`。执行代理最多把 blocker 推进到 `PROVED`；只有主线逐项审核为 `CLOSED` 且最终双轴 PASS 后，operator runbook 才可开始。

---

## 18. 主线审核后才允许执行的上线 runbook（本任务只编写，不执行）

### D1 可恢复备份与目标确认

- 确认正式目标库；
- 进入 fail-closed 维护冻结；该冻结持续到 D5 canary 和 D6 页面验收完成；
- 停止全部 intake、task leasing、station poll/submit、scheduler、outbox/worker、API/direct runner；
- 排空并证明 active job/lease/queue/旧 outbox=0；
- 生成可恢复备份；
- 验证 backup list/restore 到独立库；
- 记录 migration 前计数和 active jobs/outbox；
- 任一失败 abort，不继续。

### D2 部署暗态安全/worker

- 入口仍 V1；
- 只部署固定 SHA 的 `pre-cut dark artifact` 和数据库安全 expand；严禁部署 hard-cut artifact；
- worker 不消费生产新数据，或处于明确 disabled；
- 运行健康/权限探针。

### D3 九工位升级

- 维护冻结仍生效、不得派单/领取/提交；逐台离线安装固定 SHA V2-only 包；
- station/authorization/account/login/version/protocol probe；
- 九台全部通过才继续；
- 安装期间禁止连接活跃 V1 ingress；不允许部分版本混跑；全部工位保持 submission disabled，直到 D4 hard-cut artifact、migration 和协议探针全部 GREEN。
- D3 末、D4 前必须由实际生产部署入口运行 `v2:cutover-preflight --mode pre_cutover`；结果必须从权威 reader 同时绑定 `PRE_CUT_SHA`、`HARD_CUT_SHA`、`PLUGIN_SHA`、目标数据库身份、持续 freeze token、fresh backup+restore 证明、active job/lease/queue/outbox 全零及九工位 9/9，并处于规定 freshness 窗口。缺失、跳过、乱序、复用旧结果或任一绑定事实漂移时必须 exit 1，禁止进入 D4。

### D4 维护窗口原子硬切

- D1 冻结持续；再次确认所有入口/lease/旧事件为 0；
- 应用 final migration/roles；
- 部署固定 SHA 的 `atomic hard-cut artifact`，同时启用三个 ingress 唯一 EvidenceIngress；
- 禁止 V1/V2 双写或 fallback；
- migration 无 runtime caller；
- 旧协议硬拒绝；失败时保持冻结并 fix-forward/重放同一 Evidence，禁止应用回退到 V1；cutover 前备份恢复只属于会丢 post-cut Evidence 的人工灾难恢复决策。
- final migration/roles、hard-cut artifact 与角色/协议探针全绿后，只启用固定 SHA 的 V2 durable worker；旧 V1 outbox/worker 保持停止。V2 worker 未启用/版本不匹配时 `post_cut_canary_ready` 必须 exit 1；通过该门禁后才允许 D5 allowlist canary submission。
- V2 worker 启用后、任何 canary submission 前，必须运行 `post_cut_canary_ready`；它必须证明 actual cutoverAt/HARD_CUT_SHA/roles/protocol/worker readiness/V1 stopped/freeze 全部匹配且业务分母仍为 0。该门禁 exit 1 时保持冻结并 fix-forward，禁止进入 D5。

### D5 XHS Content canary

- 冻结不解除；使用明确 allowlist station/workspace 或人工单次入口执行 controlled execution/manual/recovery，禁止开放全量 intake；
- 验证 Evidence→B2→B3→Media→read DTO；
- 对同一 receipt 证明 durable work `pending→B2→B3→visible`，且 V1 worker 处理计数保持 0；
- conflict/rejected 不可见；
- coverage/latency/dead/retry 达批准阈值。
- canary 产生首批真实非空分母后，才允许运行 `post_cut_observe`；不得用 `pre_cutover` 或 `post_cut_canary_ready` 代替持续观测。

### D6 Material 单读硬切

- 页面只读 V2 Projection；
- V2 缺失明确不可用；
- 零 fallback；
- 不删除历史结构。
- D5/D6 smoke 全绿后才解除维护冻结并恢复派单。

### D7 连续真实 48 小时

真实线上时间和真实事件才算。本切片必须证明：

- 零 post-cut XHS Content V1 新写；
- 零 K-01/Material 旧读；
- 零该切片 fallback/Evidence绕过/Media直查；
- 零跨 workspace；
- 零假 visible；
- worker 无丢失、dead 可审计；
- 九工位版本一致；
- Material 页面运行指标在阈值内。

Author/Comment/Metric、Topic/Monitor/Radar/AI 等未切消费者必须逐文件列入 excluded scope，不得消费 post-cut 新 Content 事实，也不得被本次门禁诱导越权删除。

不能用 sleep、模拟时间、历史日志或测试替代。

### D8 Release-C 申请

48h 通过后只能申请 K-01/Material 已不可达旧读取代码的隔离/删除评审。全域旧表/字段/service/helper 的 Release-C 仍要求 BLK-001～015 全部由主线 CLOSED、全部消费者完成各自切换与真实 48h，并另获删除授权；不属于本合同自动执行。

---

## 19. 成功证据硬规则

任意 `success` 必须同时成立：

```text
公开运行入口成功
+ downstream 全部必要操作成功
+ DB 持久状态完整
+ 公开读取 seam 能观察结果
+ 零部分成功
+ 零旧路径/fallback
```

明确禁止以下推理：

- TypeScript 类型 = 运行时事实；
- `as Contract` = 验证；
- mock 被调用 = 业务成功；
- enqueue 被调用 = durable；
- route 200 = Projection 完成；
- tests 文件存在 = tests 执行；
- 0 tests/skip/todo = 门禁通过；
- `git diff --check` = untracked clean；
- governance exit 1 = 0 issues；
- 旧 proof DB = 新代码证明；
- 大多数通过 = 完成；
- 一个 service 的返回值 = 完整事实链。

API、Event、DB JSON、Queue payload、跨仓数据必须 runtime validate。基础设施成功必须用真实 PostgreSQL、两个独立连接、真实事务/worker 和公开 seam 证明。

---

## 20. 最终 handoff 固定格式

最终取件必须在 `/private/tmp` 同目录先写临时文件，校验 schemaVersion 与 BEGIN/END marker 后原子 rename 为 `/private/tmp/V2-XHS-CONTENT-RELEASE-CANDIDATE-HANDOFF.md`。handoff 本体不写自引用 hash；durable 与 pickup 各自在同目录生成独立 `.sha256` sidecar，并从磁盘重新计算完整文件 hash，两者必须一致。篡改 pickup 一个字节必须失败；模拟复制中断时旧完整取件仍可读，禁止半文件。最终内容：

1. `READY_FOR_OPERATOR_CUTOVER / BLOCKED / FAILED`；
2. contract SHA；
3. 两仓固定/最终 SHA、branch/status；
4. Phase 0～11 状态；
5. 每阶段提交和实际 diff；
6. schema/migration 对照；
7. runtime caller 链；
8. RED→GREEN 证据；
9. expected/executed/missing/duplicate/unexpected test ID 对账；
10. proof DB 名、host、current_database、PostgreSQL 版本、最终计数、保留状态；
11. directed/full/lint/build/governance/cross-repo/trace/fingerprint 的命令、exit、数量；
12. 插件包路径/SHA/版本；
13. static scans：旧写、旧读、fallback、Raw 直读、Media 直查、正式库名、unsafe SQL；
14. Spec/Standards 自审和修复；
15. DECISION_REQUIRED / SOURCE_INCOMPLETE / frozen lanes；
16. 上线 runbook 与 abort/fix-forward；
17. 未执行的 push/merge/deploy/正式库/九工位/48h/Release-C；
18. 下一恢复游标。

如果关键 lane 冻结，仍必须完成全部独立工作并以 `BLOCKED` 一次性交付，不得中途等待。

---

## 21. 最后红线

本合同授权的是“连续做完可安全完成的上线前工程，并产出可信候选”，不是授权 Agent 用猜测消除未知。

速度来自：

- 一次性上下文；
- 内部 checkpoint；
- 自修而非 R1/R2 循环；
- lane 隔离；
- 真实自动化证明；
- 最终一次审核。

速度不能来自：

- 虚报测试；
- 自行选架构；
- 放松数据库安全；
- 双写/双读/fallback；
- 伪造历史；
- 把候选写成已上线。
