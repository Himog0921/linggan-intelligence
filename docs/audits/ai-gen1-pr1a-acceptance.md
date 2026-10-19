# AI-GEN1 / PR-1A 验收回执

> 状态: 一次性报告
> 最后核对: 2026-10-19
> 适用范围: #454 / PR-1A 独立候选与隔离证明
> 事实来源: 当前工作树、实际命令回执与独立收尾审查
> 冲突时以谁为准: 最终代码/实际执行；通过只限所述层级

## 范围与授权

用户正式派定底稿补齐、合成一次性 PG 测试和 Draft PR。base `612dcbea70c69bd96d3903340b10cf9ed56a0e7b`，分支 `codex/ai-gen1-pr1a-dossier`；主 Issue #454、原验收 #420 已双向关联。未调用真实模型、未处理真实媒体或采集、未动共享数据库/服务、未合并部署，也未启动实施代理。

## 实际检查

| 检查 | 实际结果与边界 |
|---|---|
| `cargo test -p linggan-evidence --lib material_ --locked` | 36 passed；含 NFC/LF/Unicode 范围、状态/哈希、FAILED 与真实 work、混合受限/等待负例 |
| `cargo test -p linggan-worker --bin linggan-note-material --locked` | 2 passed；显式范围/动作、最大合法约4.9MB计划、16MiB上限拒绝 |
| `scripts/test-research-dossier-postgres.sh --note-material` | 旧producer fixture 1、新材料 PG 6、原 dossier PG 3、只读 HTTP 1，全部通过 |
| 原 `scripts/test-research-dossier-postgres.sh` | 20 passed：底稿、创作者/主题适配、评论冻结/重建/并发、创作者 worker 与真实 Axum 路由；本机合成回环 fixture，未调用真实供应商 |
| 迁移/隔离 | 上述 fixture 从空 schema 应用含0129/0130的迁移；旧producer只需0129。成功/回滚、唯一/不可变/计数约束及历史cutoff通过；runner确认销毁自己创建的DB/container/volume |
| 格式/治理 | 定向新Rust/relayer rustfmt、两脚本 bash -n、`git diff --check`、项目治理通过；现有API/旧文件的无关格式不批量改写 |

六组新 PG 场景分别为：已有详情标签升级/重跑/回滚；内容与来源/时间/状态/快照分责；24h边界与缺作业/重试/过期租约/终态/无文字；有界重分层与当前限制；12篇与1篇查询数量、部分失败续跑/缺OCR回执；无job受限槽位与其他pending混合。work-only漂移负例还确认旧v1 revision确实不变，saved snapshot plan依内容指纹拒绝。固定cutoff后的标签投影/分层不倒灌，当前限制优先。

证据保存在 `artifacts/private/ai-gen1-pr1a/`，忽略不提交。日志含已运行命令/结果，不含DSN或真实材料。定向 runner 和兼容 runner 的重叠用例不累加冒充覆盖；filtered out 不表示已通过，默认忽略的PG由上述 `--ignored` 在隔离库实际执行。编译存在原有未使用项警告，不宣称全仓零warning。GitHub CI以最终PR元数据为准；本机结果不能替代远端CI。

初轮曾因清理后的 domain 字段、标签 JOIN 歧义而失败；后续媒体合成 fixture 缺候选URI/违反既有next_attempt_at NOT NULL、计数扩展缺public schema限定也分别修正。最终上述结果来自真实重跑，非把失败或跳过改名为成功。

## 唯一一轮收尾审查

按 AGENTS.md 运行独立只读 commit-reviewer；没有实施代理、审查面板或第二轮整审。审查发现3项P2并在本轮关闭：

1. 最大合法200×500详情计划超过旧1MiB读取上限；改为16MiB有界上限，约4.9MB合法计划及超限负例通过。
2. 无job的已受限槽位缺少媒体行，原先仍显示等待；直接复用当前限制view，精确job/槽位分责，无job/历史cutoff/混合pending PG通过。
3. relayer副作用断言被后来的同名基线遮住；首次apply/replay后立刻核原始计数，再单独测新来源/当前限制，PG通过。

主执行者自查的snapshot计划输入漂移也在同轮修正并获独立复核：事务内同时校验旧revision与计划materialFingerprint；不把work状态漂移当作同一冻结输入。审查最终未发现未处理P1/P2；它实际运行局部规则/CLI/格式/语法并读取主执行PG日志，未另建PG或连接共享服务。最终提交只在审查修正后冻结；exact head、Draft URL和远端CI另在PR/#454回执记录。

## 未证与后续

只读 API 提供新合同，既有 React 页面与研究消费者不自动切换；没有新增 UI 因而无本次 G1 页面改版。新状态在页面的最终展示及 #420 Mog G4 仍待，真实库标签差异/材料覆盖/重分层质量没有重跑。共享迁移、历史补投影/relayer、备份恢复和发布窗口须另行明确后执行。PR-1B 开发不以 Mog G4 为前置；PR-1C 的原上线与 G4 依赖保留。
