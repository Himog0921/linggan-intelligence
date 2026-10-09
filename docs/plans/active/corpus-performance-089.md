# CORPUS-PERFORMANCE-089 · 语料读取性能

> 状态: 活跃计划
> 最后核对: 2026-10-09
> 适用范围: Issue #89 的共享 Work Resource 读取、证据库查询和创作者代表作读取
> 事实来源: Mog 当前对话授权、main 7aa0b776、3000 只读基线、当前读取合同和隔离 PostgreSQL
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码与数据库证明

## 用户结果和范围

降低语料初始列表、搜索、翻页、单篇详情和创作者代表作等待；保持现有数据、页面和权限含义。一个执行者、一个候选分支，不启动额外实施 Agent；提交前按 AGENTS.md 使用只读 commit-reviewer。Claim 在私有 Issue #89；独立 worktree 为 `.worktrees/corpus-performance-089`。不合并、不部署、不迁移共享数据库，不调用模型、平台或插件。

## 基线和方法

采用 https://claude.dev/blog/how-we-made-claude-ai-faster/ 的测量、优化、验证、回归保护循环。2026-10-09 正式 3000 的本行业列表三次中位数 1583ms、ADHD 搜索 1675ms、创作者列表 1438ms。页面壳单次 19ms，详情单次 151ms。评论研究概览单次 1931ms，仅记录为另一个待定位慢点，不据此归因或扩大到研究语义实现。

当前逐篇 enrichment 有 N+1。共享 Current 的 field-wise 最新合格记录在限定作品之前扫描全库。只读 SQL 提案：首批基础查询 286.6→33.3ms，51 条候选全字段差异为 0；指定单篇基础查询 80.6→0.76ms。单次 SQL 实验不证明端到端性能，不替代历史/分页/受限语义验证。

## 表面、状态和依赖地图

| 表面 | 当前状态和含义 | 本次处理 |
|---|---|---|
| 证据库列表/搜索/下一页 | 同一 asOf；最多50条返回、200条扫描；空、失败、受限和部分仍分开 | Current提前收窄，按候选批次 enrichment |
| 精确作品详情 | 全历史逐字段最新KNOWN；历史窗口不决定当前值 | 复用同一限定范围 Current；详情合同不改 |
| 创作者代表作 | 当前领域显式refs、相同media和资格来源 | 复用批量Work Resource读取 |
| 评论研究 | 已有研究合同和受限评论通道 | 共享Current调用自然获益；不改研究语义或执行 |

依赖为 material_query_sql、work_resource_current、material_projection、social/media/fragment 读取模块；共享文档只在本候选修改，不吸收其他 worktree。UI、plugin、worker、provider、schema/migration、采集和正式领域定义禁止修改。代码中的原始数据仍不进入普通日志、Git或性能回执。

## 实施与可证伪验收

1. 先限定 domain/ref 范围再选择各字段历史Current。无正文/lane/media SQL筛选时可先选择51个身份；存在这些筛选时不得提前截断导致漏命中。排序必须保持原有 observed_at 选择优先级与分钟显示精度。
2. discovery、collection context、lane coverage/history、comment counts、author context、provenance、excerpt、slots、derivatives、cover headline、target avatar 改为批次查询；单篇和批次共享字段投影。保留各作品独立的媒体/派生/来源receipt上限，不把批次上限替代逐作品上限。
3. 隔离PostgreSQL通过公共read_work_resources/read_work_resource验证字段历史、搜索、跨领域、cursor、200扫描预算、撤回/清理、OCR退役及覆盖范围；增加不随item数量线性增长的数据库query-count guard。
4. 同样输入的baseline/candidate在只读连接下比较结果和延迟；回执仅保留聚合指标，不保存真实正文。计时为描述性样本，query-count为确定性守门。任何不同必须说明，不以速度覆盖语义差异。
5. 运行格式/编译、相关隔离PG、source/governance检查，检查diff，提交候选交付。目标为主要列表明显改善；实际数值、剩余瓶颈和未证明层在本文更新，不承诺未测P95。

## 风险和回退

危险点是提前LIMIT丢失搜索/状态命中、observed_at排序变化、覆盖历史合并、单作品边界在批次中丢失、撤回材料泄漏。用真实PostgreSQL正反样本和baseline差异检查防护。发现产品含义变化、额外migration或文件冲突停止受影响部分。回退为整个候选代码提交，不回写任何共享数据。

## 候选实现与验证回执

- 实现：Current 在字段历史投影前限定作品；无 SQL 内容/lane/media 筛选时先做 51 个身份的 keyset 选择。discovery、采集上下文、社交/覆盖历史、出处、片段、媒体、标题和头像按候选批次读取。单篇与批次共享字段判据，没有剩余由应用逐篇发起的 enrichment SQL；详情专属读合同不变。
- 媒体：仅复用当前槽所需的下载/历史物化，保留历史副本与处置判据；只读 SQL 样本314→35ms。避免全批次数量替代单作品64槽、256派生、20出处receipt上限。
- 重复读取：generic prepared plan 的 Current 查询2932ms、custom50.6ms（各51行）。两个原生Current入口使用 SQLx `.persistent(false)`，逐次计划看到实际 domain/ref/cursor；不改共享 PostgreSQL 配置。
- 同一 asOf 的最终8次成对 HTTP 样本：正式3000 1730–2100ms、中位1784ms；候选3108 275–361ms、中位295ms，下降约83.5%。过滤 `asOf/cursor/nextCursor` 后其他字段差异0；冻结接受时间窗口消除了早前一次评论lane状态差异。样本不是P95，不证明用户页面验收。
- Docker恢复后，最终候选各入口三次成对样本（中位）：列表1118→257ms、搜索1825→485ms、跨行业867→172ms、详情181→80ms、创作者1647→1270ms。第二页单次1361→183ms；所有比较除动态cursor/asOf外字段差异0。重启后首个候选列表请求1324ms，后两次257/185ms；暖态和冷启动样本分开，不以暖态承诺冷启动或P95。评论研究概览2269→2132ms，仍约2秒，不视为已解决。
- 单元96 passed / 1 ignored；候选API build通过。八个改动Rust文件的 rustfmt check、脚本bash -n、低空间拒绝反例（未调用Docker）、git diff --check和project governance通过。全workspace fmt失败涉及main既有26个文件；本候选的八个文件均通过，不把全库格式整改混入此事项。
- Docker恢复后最终完整定向证明：六组PG39条、四组API14条全部通过，脚本验证本次proof数据库/container/volume已删除。覆盖历史、UNKNOWN、时间、领域、comment访问限制、媒体处置fallback、OCR退役和cursor。命令：`./scripts/test-corpus-performance-postgres.sh`；本候选仅操作合成的一次性库。
- 回归保护：公共 `read_work_resources` seam 上1篇与50篇均19条应用SQL；保留所有业务SELECT（包括collection_*）和schema probes，仅排除统计器、事务控制和连接ping。独立commit-reviewer发现的collection计数遗漏已修复并重跑通过。新增分页/asOf/搜索晚命中/200扫描预算和双作品64/70媒体限制验证通过。

### 环境故障与恢复

早前重跑期间host空间最低约135MB；隔离PG进入recovery/EOF，Docker虚拟磁盘记录写入I/O/EXT4 journal错误，共享3000随后503。该轮失败不算通过，故障后的计时不作结论。已停止本任务3108并删除本任务incremental缓存与真实HTTP临时响应。

Mog随后明确授权重启Docker恢复数据库。常规restart卡在旧VM退出，官方`docker desktop stop --force`完成退出后restart恢复原容器，未迁移或部署候选。共享PostgreSQL healthy、`pg_is_in_recovery=false`、迁移台账116项；原3000 PID63781/cwd/txt/runtime-main仍为7aa0b776，领域GET200且50items。故障proof容器3b7faf64e2b4及精确同名volume已删除核验；其他任务容器/数据卷不清理、不代为恢复其测试。恢复后空间约14GiB，完整证明与末轮HTTP比较完成。

脚本在创建资源前检查至少2GiB空间（低空间stub反例确认Docker未被调用），记录非秘密资源名并降低专属proof容器WAL检查点软目标（非磁盘硬上限）；该配置已用于最终完整通过。独立commit-reviewer发现的计数遗漏已修复，WAL文案修正已落实，无未修复阻塞发现。部署、P95和Mog页面体验仍未证明。

## 分层状态

- 候选代码：已实施；独立只读reviewer无未修复缺陷，最终39条PG+14条API已通过。
- 只读候选性能：同一asOf最终8次改善已验证；P95与页面体验验收未证明。
- 共享runtime恢复：已验证原版本恢复；main合并、性能候选正式3000部署、Mog验收：NOT VERIFIED；当前候选未获得merge/deploy授权。


## 2026-10-09 授权发布前整合边界

Mog 随后明确授权本 PR 合并和刷新本机 3000。发布准备发现 origin/main 已合入 PR #378，目标基线现为 370db879，包含 Topic Map 0116–0119 迁移。仅产物登记的两个追加段落发生文本冲突，双方记录保留；性能实现不增加新行为。共享运行库只读台账止于 0115；没有执行迁移、停服或部署。新版运行脚本要求 worker drain 后切换，且未迁移拒绝启动。先完成整合验证与 exact-head 审查，不得用此前 API-only 无迁移批准跨过该边界。随后发现主题图谱聊天已独立取得 Mog 的迁移发布授权并执行；本包只读台账已到 0119、runtime-main 370db879 health 200。等待该安装流程完成后按当前合并部署授权顺序发布性能版本，不重复迁移。本包不会修改研究开关、预算、插件或运行新模型调用。整合后96单元和53项聚焦PG/API通过，独立源码审查无阻塞；完整LOCAL-001另行验证。
