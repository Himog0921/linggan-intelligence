# 交给开发 Agent 的执行入口

> 状态：技术设计定稿，待用户批准开发；不是已实现或已验收声明
> 交付包：COMMENT-STUDY-PRODUCTIZATION-001 · 文档版 1.0
> 核对日期：2026-09-22
> 源码基线：`main@c74d72e3d17b9d5ecfb9953de025713c47e4560e`
> 责任：本包设计由 ChatGPT 整理；开发、共享库操作、真实模型调用与发布由 Mog 单独授权
> 事实与设计：标为“现状”的内容来自固定版本源码；“规定／新增／必须”为本次目标合同

## 1. 任务

你负责在现有 clean Comment Study 上实现 COMMENT-STUDY-PRODUCTIZATION-001。先确认 Mog 的当前授权、Issue、exact head 和独立 worktree。本包是增量产品化，不是从零开发，不执行 reset，不迁移旧 V1 派生数据，不新增第二条研究链路。

## 2. 最短读取路径

所有人先读本包 [计划](../plans/active/comment-study-productization-001.md) 和 [架构](../architecture/comment-study-productization-001.md)。

数据库／后端：再读 [数据合同](../data-contracts/comment-study-productization-001.md)、[接口与执行合同](../data-contracts/comment-study-http-001.md)、[开发手册](../runbooks/comment-study-productization-001.md)。

UI：再读 [页面规格](../design/pages/comment-study-productization-001.md) 与接口响应章节，并按现有项目 UI 合同读取 LIDS。不能为了页面字段而发明第二套后端逻辑。

自动化：本轮只实现统一启动／幂等／成本／恢复接口，下一阶段 [日计划合同](../plans/active/comment-study-automation-002.md) 是衔接约束，不是本轮启动授权。

## 3. 开始时提交一次差异回执

只回答：实际head、数据库schema是否与基线一致、计划中的文件哪些已有可复用实现、哪些必须新增、是否有冲突或缺失现场证明。发现刚合并的同等能力就复用，不再写第二份。

关键真相从固定代码／schema出发：policy复用为方法版本；comment稳定键不是source_ref；StudyRun才是用户批次；prepared本来可执行；Run完成不等于Problem归并完成；原声依据来自raw不是clean；旧方法未知不倒填。

## 4. 执行规则

按P0–P5顺序做可独立验收的提交；每步完成只更新本职责文件和必要调用点。不得自动分配子Agent或并行任务。新增表仅本轮三张，不能为“方便”再加Coverage/Task/Method/Insight系统。需要扩大范围时给出源码证据、不能复用原因和最小替代方案，等待Mog裁定。

严格实现字段、NULL、CHECK、索引、锁序、requestRef、请求快照、预算和三阶段故障矩阵。SQL在冻结查询之外不能悄悄重新取另一份语境；UI不能从前100条推算全部。默认new_only不每天无限重试failed；同评论重研不增加支持人数。

两种输入合同的历史读取只是为了保留已存在事实，不允许重新启用旧写／worker路径。新的写入必须完整方法与fingerprint；既有方法未记录的run不借默认方法继续外发。

## 5. 每次提交的验证回执

报告对应T编号、真正执行的命令、PASS/FAIL/NOT RUN、失败原因、数据保留证明、尚未验证的模型质量。没有Docker／浏览器／M4运行环境时如实标NOT RUN，不能把静态断言当真实证明。未取得单独授权不调用真实模型、不写共享库、不merge、不刷新3000。

## 6. 必须停止并报告的情况

需要reset／删除历史；现有schema半初始化；用户原始数据会暴露到Git／日志；现行调用绕开新预算／fencing；共享组件由其他交付包拥有；实际root代码与手册路径实质冲突；无法确定清洗／prompt历史却准备填入当前值。只停止受影响的执行，不用这些阻塞已获准的独立文档／合成验证。

完成不是“代码写了”。完成是同一版本里的材料可查全、研究可解释、合法部分结果可读、自动化入口可复用，同时没有丢掉过去的合法成果。
