> 状态: 活跃计划
> 最后核对: 2026-09-08
> 适用范围: CI-20260907-V1 完整评论研究交付
> 事实来源: Mog 指定实施包，manifest 已验证
> 冲突时以谁为准: 用户最新决定；现状以真实代码与运行证据为准

# CI-20260907-V1 实施与交接

用户授权：按完整实施包落实代码与隔离验收。真实外发、自动启用、共享迁移、合并和部署保持分别记录。

来源合同：[唯一合同](../../data-contracts/comment-intelligence-v1.md)；外部原包 `/Users/moglenny/Downloads/comment-intelligence-v1`，13 项 SHA256 全部一致。蓝图不是生产数据。

### CI-20260907-V1 本机发布回执（2026-09-08）

- Mog本轮授权提交、推送、合并main和刷新3000。代码PR #192已合并为`cce3f60855fae0143452136a9b96daa9ac243e4a`；独立release_review对head`8d35138`给出PASS。集成后评论PG38、模型兼容50（18重叠）、新API3、治理通过。
- 发布时保留main已有0047采集迁移，评论迁移顺延0048并注册local-runtime。共享台账0048 SHA256=`2ed806e50dbc85fa3d6ceaaffcd3041136a303e9532237953355043161d57b88`与文件一致；应用前已创建加密pg_dump并校验archive目录，未重置数据库。
- 首次发布核验：API/worker/media PID为91654/91657/91659，cwd与executable均属于`~/Library/Application Support/Linggan Intelligence/runtime-main`，revision为cce3f60；health数据库/schema READY、scheduler running。后续文档回执提交会同步runtime，PID可能更新，不能把这些点时PID当永久事实。
- 四视角真实GET均成功：1054评论、15作品、1050可研究、0分析、0问题。每次单次HTTP约982–1043ms，不是P95证明，真实规模/并发性能门槛仍未完成。API的高级观察资格false；daily enabled=false；模型调用数发布前后均5，没有新模型外发。
- 已在用户现有Chrome `localhost:3000`标签刷新，确认概览/原声/用户问题/每日观察导航和原声1054条加载。本次解除的是实际3000基本访问验证，不是此前3307阻断的绕过；完整视口/缩放/键盘/写操作、真实语义质量/embedding与Mog验收仍未完成。
- 旧“未提交/未迁移/3000未切换”及0047评论编号是实施阶段历史快照，由本回执替代。此发布不将84项全部验收或T7质量门槛改成通过；Issue #190保持打开承接剩余项。发布证据另见PR #192与Issue #190回执。

## Claim 与真实基线

- 基线 origin/main `61e8c35`；独立 `codex/comment-intelligence-v1` / `.worktrees/comment-intelligence-v1`。
- 共享 checkout 正在 claude/keyword-sampling-policy，0046 及 collection_control 文件有他人变更，全部不触碰。新增迁移使用0047并在集成时再次核验。
- 单一集成人：根代理；semantic_audit 实施 packet/cleaner/daily runner 及其测试，根代理负责 schema/API/查询/问题/统计集成；前端以冻结接口实施。
- 可复用：Evidence readable seam、Work Resource、原声资产追加修订、v2按作品Pi包、日批与调用账本、LIDS壳层。
- 缺口：稳定评论身份查询、领域安全、原声发表/首次观察/分析时间、六类语义与群体计算、稳定问题和演化、词频、可比性、概览、侧面板和纠正/收藏连通。
- 当前文档 COMMENT-DAILY-001 “未进入共享运行”是历史状态漂移，旧release记录另存PR180；不据此猜当前库。

## 表面、状态与依赖地图

- 表面：/corpus/comments 默认概览；原声、用户问题、每日观察；评论非模态面板；问题内容区详情；研究/更正模态；旧assets链接；/corpus/queries保持。
- 状态：loading、empty、error保留旧结果、model_missing保留事实和历史结果、partial保留成功、incomparable保留计数、restricted不返回失效内容、unknown不等于0。
- 依赖：现有Evidence资格/作品上下文；intelligence SQL与派生结果；现有model/Pi调用账本和worker；LIDS token与shell，壳层不重设计。
- 验收：原包 D01-D16/S01-S18/A01-A12/U01-U16/O01-O17/P01-P05，共84项，各自需具体证据；真实质量和真实外发单列。

## 执行状态

| 工作包 | 本轮落实 | 验证与剩余边界 |
|---|---|---|
| T0 真实现状与复用 | 已完成包校验、基线与复用映射；独立分支实现 | 原包13项哈希一致；未把旧报告里的数量、迁移号当事实 |
| T1 统一查询与上下文 | 已实现稳定评论身份、统一scope、20/50分页、首次观察/发表时间、来源资格、作品和回复链 | 隔离PG与临时HTTP核对；真实库与浏览器仍待验收 |
| T2 多维语义与调用 | 已实现v3按作品packet、保守清洗、精确引用、逐项拒绝、角色分离、语义指纹 | 真实Pi SDK对本地endpoint协议通过；未外发真实评论，没有真实质量结论 |
| T3 稳定用户问题 | 已实现增量候选、稳定ID、定义版本、受控归并、更名/合并/拆分/撤销、人工锁 | 9项PG业务回归；运行召回采用词项fallback，向量接口有版本/维度校验，未接入真实embedding供应商 |
| T4 群体信号与变化 | 已实现六类非互斥视角、同对象立场、来源/表达多样性、DF词云、可比窗口和五类观察 | 数字由程序计算；默认高级自动观察关闭，须通过真实质量门槛后释放 |
| T5 四视角页面 | 已实现概览、紧凑原声、用户问题、每日观察；侧栏、研究与纠正弹窗；旧资产/查询保持 | API、JS语法和DOM冒烟通过；Chrome临时入口被客户端阻断，布局/键盘/缩放NOT VERIFIED |
| T6 每日观察与恢复 | 已实现23:00半开窗口、冻集、跨批复用、迟到上下文恢复、暂停/继续/失败重试、未知用量保留 | 18项PG日批回归；0047暂停旧v2授权，旧计划入口拒绝启动，重新资格通过后显式开启 |
| T7 审查与评测 | 分包独立核对、固定范围修复；84项逐项映射、离线评测器与规模基准已落地 | 自动证明见验收记录；真实人工gold、B0/B1/B2、完整浏览器、混合负载性能尚未验证 |
| T8 集成与发布 | PR #192已合并；0048共享迁移、runtime/API/worker和3000基本验证完成 | 完整业务点击与Mog验收未完成；详细事实见本页发布回执 |

## 代码定位与复现

- `database/migrations/0048_comment_intelligence.sql`：追加身份、查询view、语义工作、人工命令/版本、稳定问题、词项、准备清单和自动观察发布门槛。未修改已应用迁移。
- `crates/intelligence/src/comment_intelligence*.rs`：统一读模型、确定性观察、人工动作、问题归并与词项。`comment_packet*.rs` / `comment_daily*.rs`：v3结果合同和有界执行。
- `apps/api/src/local_web/comment_intelligence.{rs,js,css}`：API、四视角与交互；复用原有Evidence、模型设置和Pi调用账本。
- `scripts/test-comment-intelligence-postgres.sh`：新查询/人工反馈/边界/日批，随机隔离数据库与容器，退出清理；包含10万条合成原声性能断言。
- `scripts/test-model-pi-postgres.sh`：新日批与旧Pi/评论资产/材料兼容证明。旧模型执行器测试显式使用0047之前的schema，新schema关闭旧入口另有升级回归。
- `scripts/evaluate-comment-intelligence.py`：读取自包含JSONL，核对真实/合成、作品划分、三模式、精确引用、误归并/未归并及群体审查；不会因合成分数获得真实资格。

本轮自动结果、84项映射、基线失败与未验证项以 [验收记录](../../design/acceptance/comment-intelligence-v1-acceptance.md) 为准。不要把“对应代码存在”解释成全部84场景已通过。

## 下一次执行的明确入口

1. 先核对交付分支、当前main和迁移编号是否变动；只解决实际冲突，固定版本后复验受影响合同。
2. 完成真实浏览器四视角与视口/缩放验收；当前Chrome阻断不得用原型截图代替。
3. 由用户提供/确认200–300条真实评论、至少20篇作品的人工标注与分组；按作品拆分校准/留出，获得真实模型外发授权后运行B0/B1/B2。未通过时高级自动观察继续关闭。
4. 发布操作获得明确授权后再备份、共享迁移、构建切换API与worker，核对实际进程、migration hash、自动研究开关及3000用户路径。

## 收口纪律

每包完成后记录范围、文件、实际命令、失败与下一步。集中审查后只复验修复影响，不重复全包无变化审查。所有未完成项保留，不能以剩余上下文缩减已确认范围。

## 固定源码交接指纹

本轮48个变更源码/测试/迁移/脚本文件，按相对路径排序的逐文件SHA256清单再做SHA256：`1e12afcfae6b99d4fac68ee0e4c0b0df61dade56fda16905df6390c277fbbf9e`。不包含会继续追加证据的文档，也不代表Git提交；原清单为临时产物`/tmp/ci-source-final.sha256`。下一位Agent若修改任何实现，应重新建立版本与验证证据，不能继续引用此指纹作为新版本通过证明。
