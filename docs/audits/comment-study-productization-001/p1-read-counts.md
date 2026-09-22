# P1 首个代码增量：研究批次计数

> 状态: 代码候选；Rust和隔离PostgreSQL尚未执行
> 最后核对: 2026-09-23
> 适用范围: COMMENT-STUDY-PRODUCTIZATION-001 的只读计数子项
> 事实来源: main@c74d72e3 的 comment_study_read.rs 与本提交
> 冲突时以谁为准: 数据库合同§8.1、真实测试与本提交diff

## 手册对照

对应架构S08、数据库合同§8.1、开发手册P1及T44中的JOIN乘法反例。本次只修已有read_runs：先限定Run页，再分别按run_ref统计Work和Target，避免多个作品把同批Target重复计数。SQL独立文件仍由原函数include_str加载，不创建服务/ORM/第二读取系统。

返回字段和read.v1合同不改变；现有LIMIT和domain筛选保留。空Run保留且各数量为0。此项不代表已经支持全库评论检索、300目标翻页、稳定身份去重、有效知识或方法管理。

## 新增验证候选

`comment_study_productization_postgres.rs`使用既有隔离fixture与clean bootstrap，在两篇作品中构造六个目标（五种终态加queued），断言targetCount=6而不是12、各状态分别计数、空Run不消失、limit与domain过滤成立。fixture全部为明确合成材料，不调用模型。新proof脚本只创建随机测试container/volume，不访问共享开发库。

尚未运行Cargo编译/fmt、PostgreSQL脚本及全仓库治理。T44保持NOT_RUN；即使这个六目标反例以后通过，也不能将完整T44（300目标、完整分页）标为PASS。不存在数据库migration或已上线的声明。

## 下一步

继续P1的来源资格收束、确定性清洗缓存、作品/评论分页和历史覆盖；执行前先对照手册入口及当前main差异。共享数据库操作仍受P0现场证明约束。
