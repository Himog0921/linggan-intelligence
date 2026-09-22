# COMMENT-STUDY-PRODUCTIZATION-001 · P0 执行回执

> 状态: 实施进行中；P0源码/文档部分已核对，现场数据库部分未执行
> 最后核对: 2026-09-23
> 适用范围: 当前用户批准的增量产品化
> 事实来源: 用户提供的手册v1.0、GitHub main/branch/Issue读取、当前容器检查
> 冲突时以谁为准: 实际代码/数据库证据与已批准合同；不得使用旧reset授权

## 当前基线与授权

- main与手册一致：`c74d72e3d17b9d5ecfb9953de025713c47e4560e`。
- 独立分支：`chatgpt/comment-study-productization-001`。
- Issue #295已追加当前交付启动回执；旧破坏性重建正文不适用于本轮。
- 只由本执行者推进，没有建立子Issue、启动额外Agent或提高并发。
- GitHub通过Git Data API提交，当前环境没有完整本地checkout；不把这说成已经核验用户worktree。

## 手册来源固定

八份模块正文按原相对路径入库，源SHA-256由检查脚本逐项核验；模型输出三阶段Schema及开始请求合同以JSON语义不变的紧凑格式入库。原交付的来源记录、文档检查结果和T01–T54台账分别保留，原回执不得冒充本次执行。

根手册README/汇编HTML/汇编Markdown是交付包的阅读包装，GitHub以八份正文和本包入口为唯一维护源，不再复制三份可变标准。用户批准的合成原型已登记来源hash，正文尚未导入；不得把来源登记说成可在线打开原型。旧讨论与原型出现偏差时，以已批准手册字段和执行合同裁定，不以原型模拟按钮修改数据库语义。

## 已执行的检查

当前容器已执行 `python3 verify-comment-study-productization-docs.py --root productization-staging`：八份正文hash匹配、模块间链接无缺失、三阶段Schema闭集/required对应、大小均小于6144字节、T01–T54连续。结果见import-check.json。此检查针对已准备的源文件，不是完整仓库治理脚本，也不是业务验证。

GitHub端每个正文blob由Git SHA进一步与原始文件对应。原始架构分册导入时的一处文字转录差异已用原hash对应blob修正，不能保留为新技术决定。

## 尚未执行的P0现场项

未连接用户本机，故数据库migration ledger、clean schema现状、在途模型请求、用户工作树冲突、历史行数/hash基线均未验证。当前容器没有Cargo/rustc、Docker或psql；直接git网络不可用。没有运行reset、serve、migrate、模型或部署。

这些缺口阻止共享数据库改动和运行交付声明，不阻止已授权的独立文档/纯源码候选/合成测试编写。任何增量migration执行前，仍须按runbook §2–§3取得真实P0证据。

## 本轮下一步的严格边界

先处理P1已有读取中的计数乘法问题，补最小隔离反例；不改变read.v1返回字段，不把这个局部修复包装成全库分页、去重或完整P1。材料缓存、全库目录、身份回填、方法绑定与三阶段执行继续按原手册顺序推进。

所有T编号维持NOT_RUN直到对应真实证明完成。每次开发回看：[手册入口](../../runbooks/comment-study-productization-package.md) → 本阶段章节 → 对应T编号 → 实际diff。
