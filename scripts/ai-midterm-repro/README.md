# AI 中期项目离线复现

运行：`python3 scripts/ai-midterm-repro/run.py`。仅使用已缓存 Cargo 依赖，强制 `--offline`，不连接数据库或供应商。

脚本从当前工作树原字节提取 `candidate_versions`、`problem_identity`、`content_hash`、完整 `pair_support_in` 及 Problem/Decision/Resolution 类型。每段记录源文件 SHA-256、片段 SHA-256 与行号，Rust 编译运行真实函数。数据库 helper 由 `harness.rs` 中的显式内存 fixture 替代；函数控制流、hash 计算、作者判断及支持缺失的 `?` 返回保持原样。

主题测试 63/64/65/100/1000 个定义。评论测试同义异词、空白与大小写、不同障碍、同作者、不同作者、多个 signal 部分/全部有支持。本测试中的“同义”是用例设计意图，不是模型语义准确率的证明。

每次生成独立 `artifacts/private/ai-midterm-quality-002/repro/run-*/`，包含抽取后的 Rust、Cargo.lock、stdout、stderr、来源回执与自有 target。不会借用其他工作树的 target，不清理旧运行；按本机清理规则，确认无进程且 60 分钟无写入后再删除精确自有 target。日志与源码摘录不提交 Git。

证据范围：验证提取函数在合成输入下的行为，不验证 SQL 候选筛选、权限、事务、并发、完整队列、物理发送、费用或线上状态。主题完整目录进入 payload 以及 validator 调用时点仍由源码追踪佐证；这里不声称运行了整条 packet/模型链。
