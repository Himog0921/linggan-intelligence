# CREATOR-READ-PERFORMANCE-001 · 创作者首屏读取性能

> 状态: 活跃计划
> 最后核对: 2026-10-29
> 适用范围: 语料创作者目录读取、共享媒体索引及本包隔离证明
> 事实来源: Mog 当前对话确认的诊断及连续实施授权；base 2c6dbf5f
> 冲突时以谁为准: 用户最新授权、真实源码/数据库/浏览器证据；不放宽既有资格

## 用户结果与 Claim

保护车道；用户已确认本交付包标识，无额外 Issue/消息外发。协调者 Codex / root，
task-id CREATOR-READ-PERFORMANCE-001；base `2c6dbf5fffceb36fbc9298f12d8025b2871405d7`；
branch `codex/creator-read-performance`，工作树 `.worktrees/creator-read-performance`。
目标是缩短进入、刷新、分页筛选和作者详情等待，保留完整统计、同一作品匹配、当前来源资格。

独占：creator_discovery 列表读适配、对应 API、性能索引 migration、必要证明和本包记录。
共享：migration 登记/fixture、docs 索引由本协调者窄集成。
避开另一个 creator-context-read-filter 工作包正在修改的 research_dossier.rs、material_query_sql.rs、
material_social_read.rs、target_inspection 和其余消费者。通过现有标准 dossier 读取接口展开，
不吸收该包尚未批准的行为或修改其文件。
禁止：正式数据库 migration/写入、runtime-main 变更、服务重启、worker 启用、采集、模型外发、合并部署。

## 实施与验证

1. 修复已实测的 media_observation_ref 查询索引缺口和作品清单资格查询访问路径；仅写 migration，隔离库应用。
2. 完整领域 Current 与判断资格仍参与统计；有作品研究、人工调整或账号研究依据的作品照常完整读取。
   无判断依赖的作品可暂缓完整档案，仅当前页代表作品展开；正文搜索仍读取所有相关正文。
   单次 repeatable-read / as_of 内完成，不使用跨请求资格缓存，不提前截断统计或跨作品拼接条件。
3. 比较原完整读与候选 JSON 的全部业务字段；验证跨页、同作品反例、原文变化、限制与撤回、作者支持依据、未知值。
   优化后若前端剩余串行成本可忽略则保持原读取/取消语义，不引入额外缓存。
4. 隔离真实 PostgreSQL、原 creator/dossier 回归和真实只读候选浏览器，分别实测进入、刷新、分页筛选与作者详情。
   实际正式 migration/合并/部署由后续 exact-head 授权处理，不能把候选耗时写成正式上线。

诊断基线：ADHD 3274 篇研究用途作品、1516 作者；50条 API 两次19.74秒，25条21.54秒；
浏览器进入43.74秒、刷新23.15秒、作者概览0.30秒；有限样本非P95。100篇档案清单样本333次下载表顺扫，
该表17241行且只有主键；JIT和generic/custom对照无明显改善。
临时日志、汇总、只读候选脚本在系统临时目录，本包生成物不入 Git、不保存原始材料正文。

## 交付状态

候选实现与独立审阅 PASS，尚未提交/推送、合并或正式迁移/部署，Mog验收待处理。

- 预检通过；API构建/检查通过。隔离入口 `bash scripts/test-research-dossier-postgres.sh --creator-read`：
  5项creator＋9项dossier通过。124篇作品跨页、完整统计、同作品正负反例、来源更新/人工依据失效、
  页内75篇代表作仅1批inventory读取（不展开全部124篇）、两索引真实应用均已验证；临时DB/container/volume清理通过。
- 原fixture存在两处调用类型漂移及删除AI control的过期做法，已修复为合成owner持真实advisory fence，
  按pending→running→attempt receipt→terminal推进，不禁用trigger或启动真实executor/provider。
- 正式3001 vs候选3116五组GET：默认、第二页、高赞、名单外、同作品组合；除读取时刻外完整JSON相等，
  包含1516作者/3274作品和来源/判断字段。实库同作品组合为0项，非空正负语义由隔离矩阵覆盖。
  三个错误路径（未设阈值/非法筛选/不存在领域）状态与错误JSON相同。
- 候选API样本1.36–2.08秒，对应原API20.21–27.99秒；正式库未应用0182，因此不含新增索引收益。
- 浏览器样本（含工具开销，非P95）：进入2849/2442/3027ms；刷新3062/2828/2929ms；
  翻页2396/1998/2213ms；名单外筛选4291/1996/5639/2311/2095ms；作者概览270/271/275ms。
  未丢弃慢样本，筛选仍存在2.0–5.6秒波动；console error0。原前端未改，未引入资格缓存。
- 独立审阅/root/commit_reviewer PASS；亲自API check、相关单元1项、脚本语法、迁移摘要/diff通过，
  并核对上述PG/HTTP汇总。10个代码/脚本/SQL文件按排序路径+NUL+bytes+NUL的SHA256：
  `5d0c942f71346f5cda903b0a7a719bce96579391dca6af73f337411f84552699`。
- 通过既有dossier读口保留共享读取所有权，本次缩小重复Current/完整档案的适用集合；
  当前页及有依据的作品仍经标准底稿完整读取，不宣称所有重复读取已经消除。
- 正文搜索为保证全部合格片段匹配，仍保留完整档案读取；未证明其已达到普通作者筛选的耗时。
- 正式migration、最终CI、精确提交合并和部署均未发生；报告和汇总在系统临时目录，不含原文/凭据。


## 本机发布授权接续 · 2026-10-29

Mog在当前对话明确授权本候选提交、推送、合并main、执行唯一0182迁移、部署并刷新3001，
两个worker继续停用；既有AI执行器禁用也保留。发布使用原始配置、备份与实际迁移/进程/页面回读。
不扩展到另一工作包、插件重载、供应商调用或新平台采集。此段只记录授权，实际结果另留发布回执。
