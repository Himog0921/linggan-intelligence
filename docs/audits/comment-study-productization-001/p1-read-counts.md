# P1 实施记录：批次计数与目录基础

> 状态: 代码候选；Rust和隔离PostgreSQL尚未执行
> 最后核对: 2026-09-23
> 适用范围: COMMENT-STUDY-PRODUCTIZATION-001 的P1增量
> 事实来源: main@c74d72e3、PR #338及本轮实际命令回执
> 冲突时以谁为准: 已批准数据库/HTTP合同、真实测试与对应commit diff

## 首个增量：批次计数（历史记录）

对应架构S08、数据库合同§8.1、开发手册P1及T44中的JOIN乘法反例。修复已有read_runs：先限定Run页，再分别按run_ref统计Work和Target，避免多个作品把同批Target重复计数。SQL独立文件仍由原函数include_str加载，不创建服务/ORM/第二读取系统。

返回字段和read.v1合同不改变；现有LIMIT和domain筛选保留。空Run保留且各数量为0。此项不代表已经支持全库评论检索、300目标翻页、稳定身份去重、有效知识或方法管理。

`comment_study_productization_postgres.rs`使用既有隔离fixture与clean bootstrap，在两篇作品中构造六个目标，断言targetCount=6而不是12、各状态分别计数、空Run不消失、limit与domain过滤成立。该次没有编写migration。Rust和PostgreSQL未运行，T44保持NOT_RUN。

## 2026-09-23 继续：schema-phase和清洗缓存

Mog 已授权继续开发，无需其手动建表。入手重新核对main=c74d72e3、PR head=496d8351；无新增PR评论。没有访问本机数据库；容器的Git clone因DNS不可用失败，改用GitHub connector读取/提交及隔离staging核对，不冒充已有本机worktree。

### 对照手册与实际文件

- 数据库合同§3–4、开发手册§3.1：新增 `database/migrations/0102_comment_study_productization_schema.sql` 候选。恰好三张批准的新表：clean_cache、start_request、model_request；policy/run/target/membership仅增加约定字段。旧方法/指纹/匹配版本不倒填，旧原字段不UPDATE。安装pg_trgm并建立清洗检索索引。
- 架构文件地图和开发手册§4：新增既定 `comment_study_catalog.rs`，先实现 `refresh_clean_cache`。继续调用现有 `comment-clean.v2`，不存offsets、不建第二清洗器、不创建Run或模型调用。
- SQL从领域内每条稳定评论的最新已接纳版本选取，再判断正文/限制；不从UNKNOWN新版本回退旧正文。一次最多200条，最多传回16001 scalar用于判超长；完整原文哈希仍针对完整UTF-8。全库latest投影只保存元信息，不物化全库正文。
- ON CONFLICT DO NOTHING后比对hash、文本、状态和原因；不覆盖竞争写入。写入前再次检查来源限制；缓存存在不代表有研究或外发资格。读取权限JOIN仍是后续目录接口的必做项。
- HTTP合同§2：实现字面子串转义纯函数，保留1/2字中文，转义百分号、下划线和反斜杠。这不是搜索API已上线的声明。
- 新增3个Rust单元测试和4个隔离PG测试：复用清洗器、长文本、短词转义、旧Run字段保留、不可盲重跑DDL、缓存有界/幂等、最新UNKNOWN/受限不回退、并发缓存不重复。原计数用例保留。

### 迁移与执行安全边界

0102只是手册的schema逻辑阶段，**没有注册到local-runtime.sh，也没有在共享库执行**。下一阶段constraints必须补足：同领域方法校验、不可变版本/请求、新Target写约束、稳定身份回填/唯一索引、membership复合外键；views及所有Worker dispatch gate也尚未完成。不可把目前的单列revision FK误当成完整约束，也不可把dispatch_state列存在误当成旧Worker已经遵守暂停。不能单独将0102应用到正在运行旧Worker的库。

原bootstrap和已应用migration保持字节不变。本增量不创建schedule表、不配置自动计划。现有页面/HTTP/Worker尚未调用新catalog函数；只有隔离测试调用。完整目录、跨批次排重、提示词管理和页面上线仍未完成。

### 实际验证与未验证

- 本地临时staging的13项源码形状检查：PASS；范围包括恰好三表、无破坏性DDL/旧行写入、候选未注册、输入有界、先选最新、限制重核、include路径及测试数量。这不是SQL执行或Rust编译。
- `bash -n scripts/test-comment-study-productization-postgres.sh`：PASS。
- 实际尝试执行proof脚本：退出1，`Cargo is unavailable`，尚未创建Docker资源或数据库。Rust/fmt/PG/EXPLAIN均NOT_RUN。
- 固定手册正文和T01–T54台账本轮不改；T05/T07/T45/T50/T51仅增加部分测试候选，不勾PASS；T52的ledger重放证明尚未实现。
- 全仓库治理、运行入口注册、真实模型、浏览器、部署：NOT_RUN。无共享库副作用。

## 下一步

沿同一P1继续来源资格共用、父语境限制、目录keyset/服务器搜索、作品分页及历史读取，再接用户评论视图。完整schema/constraints/views与启动/外发控制集成后才注册受控升级入口。共享迁移前仍必须完成P0现场只读盘点和Worker drain；不得为了消除差异reset历史。
