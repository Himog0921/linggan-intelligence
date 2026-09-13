# Comment Research · User Voices Read API V0

状态：PROVEN IN ISOLATION

日期：2026-09-13
范围：当前评论事实的受控 HTTP 读取。不是页面、评论清洗、研究任务、模型调用、作品上下文、问题归并或变化观察的完成声明。

## 接口

    GET /api/v0/comment-research/voices?workspace_id=<required>&limit=<1..100>&offset=<>=0>

省略 limit 时为 50，省略 offset 时为 0。返回使用一个由单条 PostgreSQL SELECT 计算的总数和分页结果；排序是本地 Current Observation 的接纳时间、noteId、commentId。commentId 仅用于稳定排序，不在 DTO 中返回。

每条 voices 项仅含已证明事实：

- text：Current Observation 的原始评论文本；
- source_note_id：来源作品 noteId；
- current_admitted_at：本系统接纳该 Current Observation 的 UTC 时间，而非平台发布时间或观察时间；
- source_evidence.evidence_id 与 record_index：可以追溯到不可变来源 Evidence 的关系；
- work_context: unavailable：V0 尚未证明可供浏览器使用的作品上下文；
- research_status: not_researched：V0 尚无研究层。

接口不会输出作者、点赞、作品标题/链接、评论发表时间、回复关系、清洗状态、模型结论或趋势；这些字段在该 read model 中没有事实证明。

## 运行证明

    scripts/prove-comment-voices-read-api-v0.sh

脚本创建随机命名的独立 postgres:16-alpine 容器、随机数据库和随机 loopback 端口。它只通过当前进程环境变量将该容器连接串交给测试，结束后删除容器；不读取旧库、共享库或已有数据库 URL。

已验证：

| 场景 | 证明 |
| --- | --- |
| 三条已接纳 Current 评论 | total=3，首个 offset page 返回 2 条，第二页返回不重复的第 3 条 |
| DTO 边界 | 断言每项只有 V0 允许的六个字段，并带有 unavailable / not_researched |
| 空 workspace / 尾页之后的 offset | 前者返回 total=0，后者保留真实 total 并返回空数组 |
| 非法参数 | 缺少或空 workspace、无效/越界 limit、无效 offset 均返回 400 JSON |
| 纯读 | 各次 GET 后，Evidence、pair、identity、source record、Observation 和 Current 六张表行数保持不变 |

## 未证明、不得声称的能力

- 通过生产数据库或现有 3000 端口的真实端到端读取；
- offset 超出尾部时可区分“没有数据”和“游标已耗尽”的专门产品语义；
- Source Evidence 的用户可读展示、权限、审计授权；
- 作品上下文、研究状态、清洗结果、LLM、问题或趋势。
