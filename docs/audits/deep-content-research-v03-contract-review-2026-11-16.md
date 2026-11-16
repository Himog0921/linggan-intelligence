# Deep Content Research V0.3 合同审查

> 状态: 一次性报告
> 最后核对: 2026-11-16
> 适用范围: `LI-DEEP-CONTENT-RESEARCH-V03` 第 2、3、4、5、7、10 节与附录 A 的字段/枚举一致性；迁移、读取接口、测试与 PR 拆分方案。不含产品决策重议，不含实现、迁移执行、L2 开启、配置切换与模型调用。
> 事实来源: 当前 `main@c57e16b1`（经 `origin/main` 核对）的源码、迁移、测试、CI 选择器与只读文档；PR #588/#590 的提交与描述；本机 `bash scripts/ci-preflight.sh origin/main` 与 `node scripts/check-document-governance.mjs` 回执。未连接运行库。
> 冲突时以谁为准: 产品和优先级以 Mog 最新确认（第 1.1、1.2 节）为准；实现事实以当前 `main` 代码与迁移为准；本报告的建议在 Mog 批准实施计划前不构成实施授权。

## 1. 结论

1. **规范的 18 条事实里 17 条在 `main@c57e16b1` 上仍成立，1 条漂移。** 唯一不一致是“最新迁移编号是 0216”：当前 `main` 已到 `0221`（`0217`–`0221` 是机会卡工作包）。其余事实的 `文件:行` 见第 2 节。
2. **迁移方案基本可行，但 M1 写的激活路径已经不存在。** `0169` 把 `linggan_skill_binding` 的守卫改成只接受 `trial` 和 `manual`（`database/migrations/0169_ai_manual_skill_activation.sql:19-24`），`first_version` 会直接抛 `skill_activation_source_invalid`。首个 Skill 版本只能走手动激活；`0200` 又不允许 L2 建 trial 批次，所以平台内 trial 对比对 L2 不可用，评测只能离线做（与第 9 节一致）。
3. **Core 版本绑定可做，但现有“有修订就隐藏 Deep”是有意的临时处理，且两处发布用的是不同的 advisory lock。** `docs/data-contracts/content-card.md:27` 写明“已有 L2 发现仍关联原 Core，后续 L2 的修订版接入单独处理”，代码里 `review.revision_ref IS NULL` 是同一提交 `fec06361` 加的。要真正实现 7.1，除了改读取与保存，还要给 `publish_reviewed_core` 补 `lock_core`（现在是 `lock_work_snapshot`，键 `196`，Deep 发布用键 `197`）。
4. **材料清单的字段都能拿到，但不在 `Snapshot.images` 里。** 图片顺序、用途、SHA256 要从 `Packet.inventory` 取；完整正文要按生产端 block 顺序拼接并自行累计字符偏移（每个 block 的 `start` 都是 0，不能拿来排序）；口播与画面文字的时间在 `Fragment.structure.time`。位置一律用**字符**偏移（与 L1 一致），不能用字节。
5. **部分接受不需要改执行器通用逻辑。** 新增失败码只要进 `linggan_ai_failure_code`，`counts_as_failure` 的现有表达式就会把它按失败计（`executor/service/content_understanding_deep.rs:170-183`）。要改的是 `accept()` 的返回结构、`payload()`、去重查询和读取投影。

## 2. 事实核对表

基线 `7437781b`，复核到 `main@c57e16b1`。`7437781b..c57e16b1` 只新增了 `0217`–`0221` 五个迁移，未改动本报告涉及的源码。

### 2.1 规范第 2.1 节六条

| # | 规范事实 | 核对 | 当前位置 |
|---|---|---|---|
| 1 | L2 只发送问题、Core 摘要、知识单元、Core 证据 | 一致 | `content_understanding/deep_grounding.rs:21-45` 的 `payload()`；只组装 `question`、`core.summary`、`core.knowledgeUnits`、`evidence`、`resultSchema` |
| 2 | 校验用 `questionFocus`/`support.quote`/`presentationOrder`，持久化时丢弃 | 一致 | 校验 `deep_grounding.rs:106-141`；落库结构只有 `id/findingType/statement/basisUnitIds/evidenceRefs`（`deep.rs:21-29`、`deep_grounding.rs:133-139`） |
| 3 | 读取只返回最新一条 Deep | 一致 | `content_understanding.rs:229` 的 `latest()`；`:259` `ORDER BY revision DESC LIMIT 1` |
| 4 | 底稿只读，测试断言没有“分析/生成”按钮 | 一致 | `apps/web/.../DossierUnderstanding.test.tsx:49`、`:128` |
| 5 | L1 传图，L2 不传图 | 一致 | `executor/service/research_call.rs:31`（`prepare_in` 传空 `Vec`）；L1 在 `executor/service/content_understanding.rs:49-50` 调 `images_in` 后走 `prepare_with_images_in` |
| 6 | 历史上有一次真实 L2，不在本地运行库 | 一致（文档） | `docs/plans/active/unified-content-understanding-v1.2-execution.md` 的“Mog 确认后…一次真实 L2”段（规范写作“第 31 行”，实际在文件中部，按段落检索更稳） |

### 2.2 规范第 2.2 节十二条

| # | 规范事实 | 核对 | 当前位置 |
|---|---|---|---|
| 1 | 本地运行库无 L2 记录、开关关闭 | **未知** | 本次审查未连接运行库：本机无 `docker` 命令，无法按 runbook 只读查询。拒绝用代码推断代替数据库事实 |
| 2 | `questionFocus` 只做子串检查 | 一致 | `deep_grounding.rs:107-113`：`chars().count() < 2` 或 `!question.contains(...)` |
| 3 | 任一条不合格整次作废 | 一致 | `deep_grounding.rs:154-159` 的 `collect::<Option<Vec<_>>>().ok_or(INVALID)` |
| 4 | 模型描述可冒充原文 | 一致 | `deep_grounding.rs:115-127`：`quote`/`modelQuote`/`observation` 任一包含即通过 |
| 5 | 顺序只能在同一个 L1 文本块内证明，最多两项 | 一致 | `deep_grounding.rs:72-90`（`:85` 比较 block 与 `end <= start`）；schema `:186` `maxItems: 2` |
| 6 | L2 未强制 High | 一致 | `research_call.rs:75-82` 只对 `TaskKey::ContentUnderstanding` 检查 |
| 7 | 去重只看问题文本 | 一致 | `generate_deep.rs:57-62` |
| 8 | Skill 包机制、大小与路径限制 | 一致 | `apps/pi-adapter/src/skills.mjs:59-66`（正文 + 按字节序排序的 `<reference>`）；`research_call.rs:64-69` 追加到 system；`skill_package.rs:61-69`、`:99`、`:109-124`；`crates/contracts/src/ai_limits.json`（64 文件 / 256 KiB / 1 MiB） |
| 9 | L2 批次只允许 `interactive` | 一致 | `database/migrations/0200_topic_research_automation.sql:14`；另见 `batch_store.rs:141-142` `!trial` |
| 10 | Core 有修订时隐藏 Deep；L2 读原始 `saved_core` | 一致 | `content_understanding.rs:250-260`（`:257` `review.revision_ref IS NULL`）；`deep.rs:130-142` `current_card_in` 只读 `linggan_content_understanding_saved_core`；`0216_content_understanding_core_revision.sql` |
| 11 | `content_understanding_deep_invocation` 只允许 `manual.v1`/`model.v1` | 一致 | `0199_content_understanding_model_deep.sql:50-53` |
| 12 | 最新迁移编号是 0216 | **不一致** | 当前 `main` 最新为 `0221_opportunity_restriction_trigger_owner.sql`；装配在 `scripts/local-runtime.sh:535` 与 `apps/api/src/local_web/full_schema_fixture.rs:345` |

### 2.3 冲突清单

每条给出位置、影响与建议；**不修改 V0.3 正文**，建议文字留给 Mog 批准。

| # | 位置 | 影响 | 建议 |
|---|---|---|---|
| K1 | `database/migrations/0169_ai_manual_skill_activation.sql:19-24` 对比规范 M1、第 7.2 节“首个版本以 `first_version` 激活” | `first_version` 已不被守卫接受，激活会直接失败 | 规范改述为“首个版本以 `manual` 激活”，或另立迁移恢复 `first_version`（不建议：会放宽既有治理） |
| K2 | `main` 最新迁移 `0221`（`scripts/local-runtime.sh:535`）对比规范 2.2-12“最新是 0216” | 编号取错会与既有迁移冲突；装配三处漏一处则 schema 不一致 | 合入时取 `0222` 起，三处同时登记；本报告第 4.4 节列了清单 |
| K3 | `0199_content_understanding_model_deep.sql:5-6`（`requires_skill=false, skill_allowed=false`）对比第 7.2 节只写了 `skill_allowed=true` | 不设 `requires_skill` 时，缺 Skill 也能源码级派发（方法与 prompt 不一致）；设为 true 时迁移后到激活前 L2 全拒 | 由 Mog 决定（第 15 节问题 1）；迁移里显式写 `requires_skill`，不留默认值歧义 |
| K4 | `crates/intelligence/src/ai_foundation/executor/service/research_call.rs:75-82` 对比第 1.1 节“模型推理 High” | 现状 L2 不强制 High；改成强制后现有隔离浏览器测试用 `off` 会失败 | 改判据为 `ContentUnderstanding` 与 `ContentUnderstandingDeep`；同 PR 把 `configure_deep` 改成 `high` 并补一条负例 |
| K5 | `crates/intelligence/src/ai_foundation/batch_store.rs:244-275`（`input_hash` 含请求 UUID）对比第 4.4 节“`input_fingerprint`” | 若误用 `input_hash` 做去重键，同输入永远不复用 | 新增独立指纹；`input_hash` 保持批次身份用途（第 7.2 节） |
| K6 | `crates/intelligence/src/ai_foundation/content_understanding/generate_deep.rs:57-62` 对比第 4.4 节去重规则 | 只看问题文本会把“换材料”误判为重复，也会把“同材料不同问题”当成新请求 | 改查 `core_ref + domain_ref + input_fingerprint`；保留 `ai_dispatch_unknown` 优先 |
| K7 | `content_understanding.rs:250-260`（`:257` `review.revision_ref IS NULL`）对比第 7.1 节 | 有修订时 Deep 整体消失，与 M6“记录所用版本”不一致 | 去掉隐藏条件，按 `core_basis` 显示；文档 `content-card.md:27` 同步补一句“V0.3 起按版本显示” |
| K8 | `core_revision.rs:63`（锁键 `196`）与 `deep.rs:123`（锁键 `197`） | 修订发布与 Deep 发布互不排斥，“发布前仍为当前版本”检查有竞态 | `publish_reviewed_core` 追加 `deep::lock_core(base_request_ref)`；测试补一个并发用例 |
| K9 | `apps/pi-adapter/src/adapter.mjs:192-201`（`inputCitationId=image_${index+1}`）对比第 4.3 节“以材料 id 对应” | 清单里的图片 id 若不是 `image_N` 且与发送顺序一致，模型引用会错位 | 清单图片 id 固定为发送顺序；`materialRef` 映射写进 payload 说明 |
| K10 | `content_understanding/source.rs:91-114`（按 SHA256 去重、跳过不可用图）对比第 4.3 节“图片按作品内展示顺序编号” | 清单编号会出现空洞或与请求图数不一致 | 清单从 `inventory` 生成并标 `duplicateOf`/`unavailable`；请求只发去重后可用的图 |
| K11 | `content_understanding/deep.rs:22-29`（`Finding` `deny_unknown_fields` + 必填 `evidenceRefs`）对比第 5.5 节 | 人工发布入口若接收 v2 发现会整条拒绝；读取路径目前不受影响 | 人工入口保持 `manual.v1`，v2 走执行器；读取不做强类型反序列化，或按 `method_version` 分支 |
| K12 | `content_understanding.rs:389-392`（L1 歧义引文直接拒）对比第 5.4-2（首次出现，仅限制顺序） | 同一摘录在 L1 被拒、在 L2 可作普通 support，口径不一致 | 明确写“L2 只在顺序证明上受歧义限制”，`order_ambiguous` 与 `excerpt_not_found` 分开 |
| K13 | `0199:36-44` 对比 `0196` 的 L1 请求表同名列 `input_manifest`（`content_understanding.rs:219-220`） | 两张表同名列语义不同，易在读取/导出时混用 | 合同写明“L2 `input_manifest` 存材料清单，L1 存 Packet”；代码注释同步 |
| K14 | `ai_foundation/content_understanding.rs:89-108`（L1 有 token 估算）对比 L2 派发路径（`content_understanding_deep.rs:26-72`） | Rust 准入没有估算；但 adapter 在调用前拦 `model_input_limit`（`adapter.mjs:169-172`），`not_dispatched` 使 `counts_as_failure=false`（`executor/receipt.rs:105-107`）。**不存在供应商侧计费失败** | 把估算提前到 Rust 准入，让准入原因可见并避免创建调用记录；不要据此声称有计费漏洞 |
| K15 | 规范 2.2-9 与第 9.3 节“Skill 版本切换” | L2 不能建 trial 批次，平台内 trial 对比不可用 | 保持离线评测 + `manual` 激活；`reason` 引用评测报告（第 4.3 节） |

## 3. PR #588 的意图与“Deep 绑定 Core 版本”的实现

### 3.1 意图判定

**已证实：`review.revision_ref IS NULL` 是有意的临时处理，不是遗留缺陷。**

- 同一提交 `fec06361` 同时加入修订表、修订写入和读取侧的隐藏条件（`git show fec06361 -- crates/intelligence/src/ai_foundation/content_understanding.rs`）。
- `docs/data-contracts/content-card.md:27` 明确写：“已有 L2 发现仍关联原 Core，后续 L2 的修订版接入单独处理。”这就是 V0.3 第 7 节要处理的事。
- 代码中的实际效果比“关联原 Core”更强：修订存在时 Deep **完全不显示**，而不是“按原 Core 显示”。这是读取层的保守做法，避免“修订后的 Core + 基于旧 Core 的 Deep”拼在一张卡上。

**推测：** 作者当时没有做版本化，是因为 `light` 的改动面更大（要改请求、发布前检查、读取投影），而 PR #588 的范围只到“底稿与 Content Card 提供当前修订”。改动它需要 Mog 或当时的工程 Owner 确认；本报告只给方案，不越权改语义。

### 3.2 第 7.1 节的具体实现

| 环节 | 现状 | 实现方式 |
|---|---|---|
| 生成时取哪份 Core | `current_card_in` 只读原始 `saved_core`（`deep.rs:130-142`） | 新增 `current_card_basis_in`：按 `(base_request_ref=$core, created_at<=now)` 取最新修订，`COALESCE(revision.result, saved.result)->'card'`，并返回 `{"kind":"revision","revisionRef":…}` 或 `{"kind":"base"}`。`latest()` 的读取已经用同一个 `COALESCE`（`content_understanding.rs:244-254`），生成侧对齐即可 |
| 保存哪份身份 | 无 `core_basis` | `linggan_content_understanding_deep_request` 增列 `core_basis jsonb`，并入 `input_fingerprint`（第 7 节）；Deep 行本身不必再存，读取时经 `analysis_ref=request_ref` 关联（`deep.rs:166-178` 已是这个身份） |
| 发布前检查 | 只检查 `card_in` 仍存在（`content_understanding_deep.rs:128-135`） | 在 `settle` 的 `lock_core` 事务里重新计算当前 basis，与请求里的 `core_basis` 比较；不同则不发布，沿用 `content_understanding_publication_withheld`（`research_call::failure` 的 `withheld` 参数，`research_call.rs:126-143`） |
| 读取按版本显示 | 修订存在即隐藏（`:257`） | 去掉隐藏条件；Deep 的 `core_basis` 与当前 Core 版本分开展示。底稿显示“基于原始 Core”或“基于某次修订”；同一作品的原始 Deep 与修订 Deep 并列，按 `createdAt` 倒序 |
| 并发 | 修订发布用 `lock_work_snapshot`（键 `196`，`core_revision.rs:63`），Deep 发布用 `lock_core`（键 `197`，`deep.rs:123`） | **必须补锁**：`publish_reviewed_core` 在 `lock_work_snapshot` 之后追加 `deep::lock_core(base_request_ref)`；否则“检查仍为当前版本”存在竞态，新修订可以在检查和提交之间插入 |

读取侧的 `skillVersion` 来源：批次项控制表 `executed_skill_version_ref` 在 `research_call.rs:114` 写入，可按 `request.item_ref` 关联，不需要在请求表重复保存（与规范 4.4 的删减一致）。

## 4. 迁移方案（第 7.2 节逐项）

### 4.1 逐项可行性

| 项 | 可行 | 说明与注意 |
|---|---|---|
| `deep_request` 增 6 列 | 可行 | 表有 `BEFORE UPDATE OR DELETE` 触发器（`0199:46-47`），只拦 DML，`ALTER TABLE ADD COLUMN` 不受影响。全部可空，旧行自动满足组合约束 |
| 组合约束 | 可行 | 建议 `CHECK (research_mode IS NULL AND base_mode IS NULL AND sub_questions IS NULL AND core_basis IS NULL AND input_manifest IS NULL AND input_fingerprint IS NULL OR (research_mode IS NOT NULL AND sub_questions IS NOT NULL AND core_basis IS NOT NULL AND input_manifest IS NOT NULL AND input_fingerprint IS NOT NULL AND (research_mode <> 'custom' OR base_mode IS NOT NULL) AND (research_mode = 'custom' OR base_mode IS NULL)))`。注意“六列同时为空”的写法要显式括号，避免 `OR` 优先级把旧行判失败 |
| 索引 `(core_ref, domain_ref, input_fingerprint)` | 可行 | 与去重查询一一对应（第 7 节）；现有 `content_understanding_deep_request_core (core_ref, domain_ref)`（`0199:45`）保留 |
| `deep` 增 `research_mode`/`sub_question_status`/`rejected` | 可行 | 表同样有不可变触发器（`0198:12-13`），只拦 DML。`rejected` 存 `jsonb` 数组即可，读取不投影 |
| `content_understanding_deep_invocation` 允许 `model.v2` | 可行 | `DROP CONSTRAINT content_understanding_deep_invocation` + `ADD CONSTRAINT`，三条分支：`manual.v1`（`invocation_ref IS NULL`）、`model.v1`/`model.v2`（`invocation_ref IS NOT NULL`） |
| 新增失败码 | 可行 | `INSERT INTO linggan_ai_failure_code VALUES ('content_understanding_deep_rejected_excess',false,true,'…','executor_v1')`。`retryable=false` 与“同一输入重跑会得到同样结果”一致；`linggan_model_invocation.failure_code` 有外键（`0138:175`），**不插会写失败** |
| `skill_allowed=true` | 可行 | `linggan_ai_task` 无不可变触发器；`0169:17` 的技能绑定守卫要求它为真，`task_config.rs:185` 也要求 |
| prompt 版本 2 | 可行 | `linggan_ai_prompt_version` 有 `UNIQUE(task_key, version_no)` 与 `octet_length BETWEEN 1 AND 24000`（`0170:11-21`）；迁移里插 `version_no=2` 即可，不推进 `linggan_ai_task_setting`，当前配置仍用版本 1 |
| 不切换配置 | 正确 | 迁移只加行；切换模型、Skill、prompt 是 `save_task_config`/`activate_skill`/`activate_prompt` 的运行时命令，需在 AI 设置里由人执行 |

补充：`0199:5-6` 建任务时就写了 `requires_skill=false, skill_allowed=false`。`skill_allowed=true` 必须由新迁移显式放开，否则 `activate_skill`/`switch_skill` 会被 `task_config.rs:185`、`skill_binding.rs:73`、`0169:17` 依次拒绝。

### 4.2 `requires_skill` 取 true 还是 false

| 取值 | 对准入的影响 | 建议 |
|---|---|---|
| `true` | 配置里 `skill_version_ref` 为空时，`admission/base.rs:51-53` 直接拒绝 `skill_not_configured`；在“迁移已上、Skill 尚未激活”的窗口里，L2 一律不可派发 | **推荐 true**。L2 的方法正文只在 Skill 里，没有 Skill 就等于跑一个缺方法的 prompt v2，失败会伪装成模型问题。代价是迁移后到激活前 L2 完全不可用（当前本来也是关闭的，可接受） |
| `false` | 未激活时也能派发，靠 `skill_allowed=true` + 手动开关控制 | 只在“prompt v2 自带完整方法”的过渡方案里成立；本规范删除的是方法正文，不成立 |

注意：`true` 会影响现有隔离测试——`apps/api/src/local_web/content_understanding_browser_tests.rs:141-172` 的 `configure_deep` 不设 `skill_version_ref`；迁移上线后该测试要么先建并绑定 Skill，要么该用例改为断言 `skill_not_configured`。

### 4.3 首个 Skill 版本的激活路径

**已证实：`first_version` 已不可用。** `0169_ai_manual_skill_activation.sql:19-24` 的守卫只接受 `trial`（需 trial 批次）与 `manual`，其余抛 `skill_activation_source_invalid`；Rust 侧 `skill_binding.rs:143-163` 也只产出 `trial`/`manual`。

可行路径（二选一，均为运行时操作，不是迁移）：

1. `save_task_config`（带 `skill_version_ref`）→ 生成 candidate 配置（`applied=false`，`task_config.rs:206-218`）→ `activate_skill(candidate_config_ref, trial_comparison_ref=None)`。
2. `switch_skill(version_ref)`（`skill_binding.rs:43-93`）：直接校验 `skill_allowed` 并新建配置 + 更新绑定。

两条路都要先 `import_skill` 上传 `content-deep-research` 包（`skill_store.rs:29-118`），且 `SKILL.md` 必须是 NFC、无 `\r`、路径不含 `scripts` 段（`skill_package.rs:61-69`）。

**trial 批次**：`0200:14` 与 `batch_store.rs:141-142` 都禁止 L2 建 trial，`linggan_ai_trial_qualifies`（`0136:29`）因此对 L2 永远不成立。结论：L2 的 Skill 版本切换只能靠离线评测集 + `manual` 激活，规范第 9.3 节“切换记录写进 `reason`”是唯一可行的留痕方式。

### 4.4 迁移编号与装配（共享文件只由一个 PR 改）

- 编号：`0216` 已过时，当前最新 `0221`；合入时取 `0222+`。
- 装配有**三处**，漏一处 schema 就不一致：
  1. `database/migrations/02xx_*.sql`；
  2. `scripts/local-runtime.sh` 的 `apply_migration_once`（`:535` 之后）；
  3. `apps/api/src/local_web/full_schema_fixture.rs` 的 `FULL_MIGRATIONS` `include_str!`（`:345`）与末尾 ledger 的 `migration_sha256`（`:501` 起的 `INSERT`）。
- 摘要用 `shasum -a 256 database/migrations/02xx_*.sql`。仓库已有“fixture 摘要必须等于真实文件”的测试范式（`crates/intelligence/tests/comment_like_library_postgres.rs:31-52`），新迁移建议照抄一个，否则 ledger 手抄错误只在运行期暴露。
- CI 路由：`scripts/ci-select.mjs:61-70` 只列了 `0196/0198/0199`；新迁移文件属于未知路径，会按“未知路径选全部 proof”处理。若希望定向路由，同 PR 更新选择器与其测试（`scripts/ci-select.test.mjs`）。

## 5. 材料清单：第 4.3 节字段可得性

来源都是 `source::Packet`（`content_understanding/source.rs:7-11`），但需要分清三个入口：`snapshot.texts`、`snapshot.images`、`packet.inventory`。

| 第 4.3 节字段 | 现状 | 缺口与最小补法 |
|---|---|---|
| 完整正文 `body.text` | `snapshot.texts` 含全部 `body` 片段，`text` 是单个 block 原文；每个 block 的 `start=0`、`end=chars`（`crates/evidence/src/research_dossier/projection.rs:414-420`；`crates/evidence/src/creator_discovery.rs:31-49`），**不是累计偏移** | 正文通常是多个 block，不是一条。清单生成时按 `snapshot.texts` 中 body 项的生产端顺序拼接，并由清单自行累计字符偏移；不要按 `start` 排序，也不要用“前块 end = 后块 start”推断连续性（它们永远对不上）。覆盖缺口要看 `coverage`，不要从坐标推断。L1 的 `model_draft::material` 直接下发 `texts` 数组（`content_understanding/model_draft.rs:8-19`），没有合并，不能照抄 |
| 口播时间 `timeStartMs/timeEndMs` | 在 `Fragment.structure.time`（证据：`crates/evidence/tests/research_dossier_postgres.rs:628`、`apps/api` 侧同类 fixture `crates/evidence/tests/support/standard_dossier_fixture.rs:115`）；`source.rs:67-73` 保留了 `structure`（只删 `text`） | 清单生成从 `structure.time.startMs/endMs` 取；缺失就是 `null`，不能推断。注意 L1 的 `start/end` 是字符位置，**不是**时间 |
| 画面文字时间 | 同上（`frame_text` 片段带 `structure.time`） | 同上；`frame_text` 若来自 OCR 且无时间，只能 `null`，且不得用于 5.4-4 的顺序证明 |
| 图片展示顺序 | `packet.inventory` 有 `displayOrderState`、`page`(=display_ordinal)、`producerOrdinal`（`content_understanding/source.rs:84-90`；生产见 `crates/evidence/src/research_dossier/inventory.rs:68-84`） | `Snapshot.images` **没有**顺序与用途字段（`:105-113` 只有 `citation_id/source_ref/source_version/blob_sha256`）。清单必须从 `inventory` 生成，用 `blobSha256` 或 `sourceRef` 回连 `snapshot.images` 的下标。`displayOrderState=UNKNOWN` 时按 4.3-2 写 `unknown`，不编号 |
| 图片 SHA256 | `blobSha256`（`inventory.rs:76`；`source.rs:97`） | 已在 `Snapshot.images.blob_sha256`，可直接用 |
| 封面与正文图区分 | `inventory.purpose` 有用途；`Snapshot.images` 已丢掉用途 | 清单从 `inventory.purpose` 判定 `cover`/`image`；不要把 `snapshot.images[0]` 默认当封面 |
| 重复图片 | `source.rs:91-114` 按 `blobSha256` 去重，只保留首个 | 清单若按 `inventory` 全量编号，会出现“清单有、请求没有”的图片。要么清单也只保留去重后的图，要么在清单条目上标 `duplicateOf`。二选一必须写进合同，否则 `materialRef` 与 adapter 的 `image_N` 对不上 |
| 不可用图片（restricted / 未物化） | `inventory` 标 `available=false`，`source.rs:93-95` 跳过；`images_in` 若物化缺失会直接报错（`source.rs:191-209`） | 清单不能列“不可用图”。发布前必须重新核对物化（`images_in` 已有校验），缺失时不发布，不能静默少图 |
| `core_unit` / `core_evidence` | 来自本次使用的 Core 版本（第 3 节）；L1 `model_read` 证据带 `modelQuote`/`observation`（`deep_grounding.rs:36-39`） | 需要按第 3 节的 basis 取修订版 Card；`evidenceKind: "model_read"` 由清单生成时补 |

## 6. 位置与顺序（第 5.4 节）

**已证实：L1 的位置一律用字符偏移。** `content_understanding.rs:529-540`：

```rust
let (byte, quote) = quote_in_source(&source.text, &candidate.quote, …)?;
let start = source.start.checked_add(source.text[..byte].chars().count())…;
let end = start.checked_add(quote.chars().count())…;
```

**注意坐标系的粒度：`start` 是“块内”字符偏移，不是全文偏移。** `crates/evidence/src/research_dossier/projection.rs:414-420` 为每个 block 生成的 `Fragment` 都是 `start: 0, end: text.chars().count()`，`source.rs` 原样复制。L1 的 `inputCitationId` 标识 block，`start/end` 相对该 block 的 `text`，所以：

- 同 block 内的先后（`deep_grounding.rs:72-90` 的 `textPosition.block` 比较）用块内偏移是对的；
- 跨 block 的先后不能拿两个 `start` 比较——它们都会是 0。5.4-2 若要覆盖“合并后的完整正文”，必须在 manifest 生成时按**生产端 block 顺序**自行累计字符偏移，并把 L1 证据的 `(inputCitationId, start, end)` 映射到合并坐标。

因此 5.4 必须写“**字符**偏移”，实现时用 `chars().count()` 转换 `match_indices` 的字节位置；直接比较字节会在大段中文里提前判定先后。

**摘录多次出现：**

- L1 的规则更严：`quote_in_source` 一旦找到第二个候选就返回 `invalid_text_quote`（`content_understanding.rs:389-392`），即**不允许**歧义引文。
- 规范的 5.4-2 是“取首次出现；出现多次则该条不能用于顺序证明”，只约束顺序，不拒绝发现。
- 结果：同一段话在 L2 里能作为普通 support，在 L1 里会整条被拒。两者用途不同，可以并存，但必须写清：**L2 的 `excerpt_not_found` 只判“找不到”，`order_ambiguous` 才判“多次出现”**，不能把 L1 的拒绝语义混进来。
- 另需明确：L1 对口播允许“逗号 + 换行”的排版等价（`content_understanding.rs:321-352`、`:361-388`）。L2 若不复用这套等价规则，从 ASR 复制的摘录会因换行被判 `excerpt_not_found`。建议 v2 复用同一函数，或在 Skill 里明确要求“按提供的 text 逐字复制、不跨换行”。

## 7. 指纹（第 4.4 节）

### 7.1 规范化方法（建议写进合同）

1. 构造一个固定形状的对象（字段顺序固定，不依赖 `serde_json::Map` 的迭代顺序）：
   ```json
   {
     "contract": "content-understanding.deep-input.v1",
     "researchMode": "…",
     "baseMode": "…",            // 仅 custom，否则 null
     "subQuestions": [{"id":"Q1","text":"…"}],
     "methodVersion": "content-understanding.l2.model.v2",
     "coreBasis": {"kind":"base"} | {"kind":"revision","revisionRef":"…"},
     "materials": [ {"id":"M03","materialKind":"body","sourceRef":"…","sourceVersion":"…","displayOrder":2,"orderState":"known","sha256":"<规范化文本的 sha256>","mediaSha256":null,"timeStartMs":null,"timeEndMs":null} ]
   }
   ```
2. 规范化：文本做 NFC + `\r\n`/`\r`→`\n`；不做其他归一化（不去标点、不 trim 句子内部）；`excerpt` 不参与指纹（它是模型输出）。
3. 图片只计 `mediaSha256` 与顺序，不计图片字节本身；正文计规范化后的文本哈希，不重复存全文。
4. `sha256(json字符串)`，十六进制小写，复用 `research_text::content_hash`（`crates/intelligence/src/research_text.rs:6-11`）。
5. 明确排除：`requestRef`、`batchRef`、`itemRef`、任何时间戳、UUID、执行配置引用。这些进指纹会让“同输入”永远不相等。

### 7.2 与现有哈希的关系

| 现有机制 | 实际含义 | 与 `input_fingerprint` 的关系 |
|---|---|---|
| `batch_store::input_hash`（`batch_store.rs:244-275`） | 对 `BatchInput` 列表（`item_key` + `InputRef{request_ref}`）做 SHA256 | **不能当内容指纹**：它包含请求 UUID，每次生成都不同，只用于批次身份与幂等。不要复用，也不要替换 |
| `generate_deep` 去重（`generate_deep.rs:57-62`） | 按 `core_ref + domain_ref + question` 查 pending/running 或 `ai_dispatch_unknown` | 改为 `core_ref + domain_ref + input_fingerprint`，保留 `ai_dispatch_unknown` 的优先返回与 `ai_prior_dispatch_unresolved` 拒绝（`:63-70`）。旧记录 `input_fingerprint IS NULL` 不参与新去重，避免把 v1 误判为同输入 |
| `Packet::hash()`（`source.rs:12-18`） | L1 材料快照身份 | 可作为 manifest 文本段的上游来源，但 L2 指纹还要含子问题、模式、Core 版本；不要直接拿它当 Deep 指纹 |

去重语义：同指纹且仍在执行 → 复用；同指纹已成功 → **不复用**（规范没说复用已完成结果），新请求照常入队，读取时自然出现多条。这一点建议在合同里写明，避免实现时把“已完成复用”当成优化。

## 8. 图片能力（只读核对）

| 问题 | 结论 | 依据 |
|---|---|---|
| Rust 侧能否带图 | **能** | `PiImage { mime_type, data(base64) }`（`crates/intelligence/src/pi_adapter.rs:14-19`）；`prepare_with_images_in` 写入 `request.images`（`research_call.rs:33-39`、`:89`）；L1 已走通（`executor/service/content_understanding.rs:49-50`） |
| adapter 是否接受多图 | **能，且有单图上限** | `apps/pi-adapter/src/adapter.mjs:77-86`：数组非空；`analyze` 可以是多张，`analyze_image` 必须恰好 1 张；单图 base64 ≤ 10,666,668 字符、解码后 ≤ 8,000,000 字节，并校验 PNG/JPEG/WEBP magic |
| 张数上限 | **代码里没有硬上限**，但有两道间接限制 | ① `adapter.mjs:171` 对 qualified 模型按每张 1024 tokens 预扣：`estimateTopicInputTokens + images*1024 > inputTokenLimit` 即拒绝；② 请求体总大小 `maxRequestBytes=16 MiB`（`ai_limits.json`） |
| 图片如何编号 | **由 adapter 按数组下标注入** | `adapter.mjs:192-201`：`inputCitationId=image_${index+1}`，并注明只对下一张图生效。所以清单里的图片材料 id 必须与发送顺序一致，建议直接用 `image_1…image_n`，或保证映射可逆 |
| execution 策略 | **必需** | `adapter.mjs:78` 要求带 `execution`；`transport_store.rs:93-108` 的 `model.request` 已自动带 |
| 当前 L2 模型配置是否支持图片 | **未知** | 运行库不可达（本机无 `docker`），无法核对 L2 的 `model_config_ref`、`capability_ref` 是否声明图像输入，也无法核对供应商是否接受。历史真实 L2 使用 `deepseek-flash`（`unified-content-understanding-v1.2-execution.md`），该模型是否接受图像输入必须实测/查供应商能力，不能推断 |
| 外发许可 | **不需新增类别** | L2 的 material_class 固定为 `note_material`（`admission/base.rs:164-168`），`0199:25-34` 已允许；图片随 `note_material` 走，不是新许可 |

建议：把“图片上限 9 张”（规范 4.2）实现为清单生成的硬上限，并在 G2 第 5 步前补一次只读能力核对（模型 capability 的输入类型 + 供应商文档），不发起调用。

## 9. 部分接受与回执（第 5.6 节）

### 9.1 现状接线

`accept()` 返回 `Result<Value, &'static str>`（`deep_grounding.rs:143-164`）。执行器的 settlement（`executor/service/content_understanding_deep.rs:112-187`）是：

```rust
let success = accepted.as_ref().is_some_and(Result::is_ok);
let failure = accepted … .err().copied()
    .or_else(|| research_call::failure(response, allowed, success, "content_understanding_publication_withheld"));
…
counts_as_failure: !success && allowed && !receipt::not_dispatched(response)
    && failure != Some(generate_deep::INSUFFICIENT_EVIDENCE)
```

- `accept()` 返回新的 `Err("content_understanding_deep_rejected_excess")` 时，`failure` 直接取它；`counts_as_failure` 为真（因为不是 `INSUFFICIENT_EVIDENCE`）。**不需要改执行器通用逻辑。**
- `receipt::write_item_in` 只负责落 `state='failed'`、`failure_code`、`counts_as_failure`（`executor/receipt.rs:129-143`）。
- 失败码必须先进 `linggan_ai_failure_code`：`linggan_model_invocation.failure_code` 有外键（`0138:175`），否则 `finish_invocation_in` 会写失败，回执也落不下去。

### 9.2 需要改的地方

1. `accept()` 内部从“遇到第一条不合格就 `None`”改为“逐条产出 `Accepted` / `Rejected{code}`”，最后按 5.6 判定：
   - JSON/schema 不合法 → `ai_output_invalid`（保持）；
   - 合格 ≥ 1 且 `rejected_count * 3 <= total` → 发布合格项 + `rejected` 数组；
   - `rejected_count * 3 > total` → `content_understanding_deep_rejected_excess`；
   - 0 合格且全部子问题 `insufficient` → 现有 `content_understanding_deep_insufficient_evidence`；
   - `answers` 指向 `insufficient` 子问题 → 该条拒绝（`answers_unknown`），不升级为整次失败。
   - 整数边界：用 `rejected * 3 <= total` 表示“不超过 1/3”，与规范“超过 1/3 扣下”一致；测试要写“3 条拒 1 条发布、3 条拒 2 条扣下”。
2. 返回值需带 `rejected` 与 `subQuestionStatus`，`publish_in` 多写两列（`generate_deep.rs:166-178`）。
3. 被拒原因代码是封闭枚举（规范 5.6），建议在 Rust 侧用 `enum RejectCode` + `serde(rename_all="snake_case")`，避免字符串拼写漂移；Skill 侧只把它当说明用，不参与校验。
4. `content_understanding_deep_insufficient_evidence` 现在从 `deep_grounding.rs:6` 导出，`generate_deep` 再导出（`:12`）；新增码放在同一处，保持单一来源。

## 10. High 强制（第 1.1 节与 C12）

**已证实（与工作单提问的前提相反）：当前 `research_call.rs:75` 只对 L1 强制 High，L2 没有强制。** 规范 C12 的描述是对的；工作单“对 L2 也强制 High”是待实现目标，不是现状。

改成 L2 也强制后，影响：

| 影响面 | 具体位置 | 后果 |
|---|---|---|
| 现有隔离浏览器测试 | `apps/api/src/local_web/content_understanding_browser_tests.rs:163-169` 的 `configure_deep` 用 `thinking_level: "off"`、`input 8192 / output 1024` | 派发前被拒 `content_understanding_requires_high`，`prepare` 返回 `AdmissionDenied`，执行器走 `defer`（`content_understanding_deep.rs:85-95`），测试里 `deep_calls`/`deep_counts` 断言失败 |
| L1 侧 | `ai_foundation/content_understanding.rs:89-108` 的准入已在 `reasons()` 里查 High | 无需改；但两处检查的措辞要统一（现在一个 `content_understanding_requires_high` 用于 L1 preview，一个用于派发） |
| 能力声明 | `linggan_model_capability.thinking_levels` + `linggan_validate_ai_model_config`（`0134:13-33`） | L2 的模型配置必须声明 `high`，否则保存配置时就被拒 |
| 运行时配置 | `linggan_ai_task_config` 的 L2 当前配置 | 切换需在 AI 设置里新建配置，不是迁移 |
| 失败语义 | `defer` 会一直挂起等待 | 运维只看到“等待中”。建议在 AI 设置的任务状态里显示具体原因，或在验收清单里要求先确认 `high` 再开启 L2 |

## 11. 读取接口（第 7.3 节）

### 11.1 现状与依赖

- 读取 SQL：`content_understanding.rs:229-293`。Deep 是 `LEFT JOIN LATERAL … LIMIT 1`（`:255-260`），结果被塞进 `analysis.result.card.deep` 与 `analysis.deepQuestion`（`:279-285`）。
- 前端唯一消费者：`apps/web/src/pages/corpus/evidence/DossierUnderstanding.tsx:24-40`（`readDeep`）、`:52`（调用）、`:85-96`（渲染 `deep.question`/`deep.findings`/`deep.evidence`）。类型定义在 `:10-11`。
- **现有前端测试没有 Deep 夹具**：`DossierUnderstanding.test.tsx` 全文没有 `deep` 字样，只有“无按钮”断言（`:49`、`:128`）。改成 `deepAnalyses[]` 不会打红现有测试，但也意味着**当前没有任何 UI 回归覆盖**，必须新增用例，不能靠“旧测试通过”当证据。
- 后端读取不做结构反序列化：`findings` 以 `Value` 原样返回。规范担心的 `deny_unknown_fields`（`deep.rs:22-29`）只作用于**人工发布入口** `PublishDeep`，不在读取路径。因此 v1/v2 共存不强制“分开反序列化”；但若后续给读取加类型，v1（`evidenceRefs`）与 v2（`answers/support/order`）必须分版本解码。

### 11.2 变更清单

| 变更 | 位置 | 说明 |
|---|---|---|
| `analysis.deep` → `analysis.deepAnalyses[]` | `content_understanding.rs:229-293` | 去掉 `LIMIT 1`，按 `created_at DESC, revision DESC` 返回全部；每条带 `analysisRef/coreRef/coreBasis/domainRef/researchMode/baseMode/question/subQuestions/subQuestionStatus/methodVersion/skillVersion/findings/materials/createdAt` |
| `materials` 只投影被引用的条目 | 同上 | 从请求的 `input_manifest` 取，按 findings 的 `materialRef` 过滤；不要返回全文，避免读取接口变成大对象 |
| `skillVersion` | 同上 | 经 `request.item_ref → linggan_ai_batch_item_control.executed_skill_version_ref` 关联（写入见 `research_call.rs:114`） |
| `asOf` | 同上 | 现有 `$5` 截点同时作用于 Core 修订与 Deep（`:249`、`:252`、`:258`）；新查询保持“每条 Deep 按自身 `created_at<=cutoff`” |
| `rejected` 默认不返回 | 同上 | 读取投影不选该列；评测导出另开接口或离线 SQL |
| 旧记录兼容 | 同上 | `researchMode=null`，`deepAnalyses` 长度 1；`materials` 为空数组（v1 没有 manifest，不补造） |
| `GET …/deep/modes` | `apps/api/src/local_web/content_understanding_api.rs:13-26` | 新增路由；返回方法版本、三个模式、默认子问题、默认材料规则；常量放后端（规范 3.3） |
| 前端类型与展示 | `DossierUnderstanding.tsx:9-18`、`:24-52`、`:85-96` | `Deep` → `DeepAnalysis[]`；列表按模式/子问题分组；显示 `coreBasis` 徽标（“原始 Core”/“某次修订”）；`insufficient` 子问题显示缺口 |
| 浏览器测试 | `apps/api/src/local_web/content_understanding_browser_tests.rs` | 现有用例只断言计数；新增断言读取投影（含 `deepAnalyses` 顺序与 `asOf` 截点） |

## 12. Skill 正文长度

按附录 A 的五个代码块实测（本报告生成时用脚本提取，未含 `<reference>` 包裹标签）：

| 文件 | 字符 | 字节 | 中文字符 |
|---|---:|---:|---:|
| `SKILL.md` | 2,061 | 4,273 | 946 |
| `modes/content-design.md` | 504 | 1,204 | 284 |
| `modes/content-value.md` | 540 | 1,250 | 307 |
| `modes/creation-method.md` | 703 | 1,481 | 325 |
| `references/finding-types.md` | 795 | 1,795 | 413 |
| **合计** | **4,603** | **10,003** | **2,275** |

- 拼接后约 10 KB 字节，**远低于**包上限（单文件 256 KiB、合计 1 MiB，`ai_limits.json`），也低于 prompt 表 24,000 字节约束（虽然 Skill 不入 prompt 表）。
- Token 数：**推测** 约 2.3k–4.6k tokens（中文 1 字约 0.5–1 token，加上少量 ASCII），与规范 9.5 的“3–4k”同量级。**必须以实际供应商 tokenizer 首次实测为准**，本报告不把它当作已验证数字。
- **风险（已修订）：Rust 准入没有 token 估算，但 adapter 有外发前检查。** L1 在 `ai_foundation/content_understanding.rs:89-108` 用 `estimate_input_tokens(system+body) + images*1024` 与 `input_token_limit` 比较；L2 的 `prepare` 与 admission 都没有这一步（`content_understanding_deep.rs:26-72`、`admission/base.rs`）。但请求带 `execution`（`transport_store.rs:93-108`），进入 adapter 的 qualified 分支后，`adapter.mjs:169-172` 会在**调用模型之前**用同一 1024/图 的保守估算拦下并返回 `model_input_limit`；`rejected(observation)` 置 `requestSent=false`（`adapter.mjs:227-231`、`diagnostic.mjs:32-34`），Rust 侧 `receipt::not_dispatched`（`executor/receipt.rs:105-107`）因此把 `counts_as_failure` 置为假，`observe_in` 把 `dispatch_state` 记为 `not_dispatched`。**所以不会产生供应商调用，也不会计为失败。** 仍建议把估算提前到 Rust 准入：可以避免创建调用记录与 `ai_provider_failed` 这类误导性的失败码，并让准入原因在任务状态里可见；这不是“缺少外发前保护”，不要把实施计划建立在不存在计费漏洞上。

## 13. 测试方案

全部使用合成材料；不使用真实笔记正文、图片或视频。

| 场景 | 文件 | 夹具/方式 |
|---|---|---|
| v2 accept：每条拒绝码至少一例（`unknown_material`…`basis_cycle`） | `crates/intelligence/src/ai_foundation/content_understanding/deep_grounding/tests.rs` | 现有 `card()`/`finding()` 合成卡（`:5-25`）扩展成 v2 `materials`；纯函数，无 DB |
| 整次判定：1/3 边界（3 拒 1 发布、3 拒 2 扣下）、全 insufficient | 同上 | 同上 |
| 顺序：同块递增、摘录多次出现（`order_ambiguous`）、跨材料 known/unknown、口播时间 | 同上 | 合成 manifest，含已知/未知 `displayOrder` 与 `timeStartMs` |
| `supportType` 不接受模型自填 | 同上 | 输入里写 `supportType` 应被 `deny_unknown_fields` 拒 |
| 方法：无 basis、basis 指向后出现、成环 | 同上 | 合成 findings |
| 断言性质判定树（4 种 + engagement_hypothesis） | 同上 | 合成 statement/support |
| 请求 v2 落库、指纹、去重（同指纹复用 / 不同材料不复用 / `ai_dispatch_unknown`） | `crates/intelligence/tests/content_understanding_executor_postgres.rs` | 隔离 PostgreSQL；沿用 `start_from`/`submit_body` 模式（`:748-766`） |
| 发布前指纹重算不一致 → `content_understanding_publication_withheld` | 同上 | 构造材料快照变化 |
| Core 修订：生成用修订版、发布前出现新修订 → 不发布、读取按版本显示 | 同上 | 先写一份修订（`publish_reviewed_core`），再生成/发布 |
| 迁移：新增列/约束/失败码/索引、旧行导出、fixture 摘要 | `crates/intelligence/tests/*_postgres.rs`（新建或并入现有） | 参考 `comment_like_library_postgres.rs:31-52` 的摘要测试；隔离库跑迁移 |
| 端到端（HTTP + 队列 + 合成供应商）：多张 Deep、`asOf`、`deepAnalyses` 顺序、图片发送 | `apps/api/src/local_web/content_understanding_browser_tests.rs`；入口 `bash scripts/test-research-dossier-postgres.sh --dossier-ui`（`--ai-readonly`/`--ai-service` 不跑这个用例） | 现有 `configure_deep` 需改成 `thinking_level: "high"` 并绑定 Skill；合成端点返回固定 JSON |
| 前端：`deepAnalyses[]` 渲染、两种 `coreBasis` 徽标、`insufficient` 子问题、无“分析/生成”按钮 | `apps/web/src/pages/corpus/evidence/DossierUnderstanding.test.tsx` | 新增带 `deepAnalyses` 的 payload 夹具（现在完全没有） |
| 路由：新迁移/新文件命中正确 proof | `scripts/ci-select.test.mjs` | 若改 `ci-select.mjs` 则同步 |
| 文档：索引、链接、状态头 | `node scripts/check-document-governance.mjs` | 本报告 PR 自查 |

遗漏补充（规范第 10 节没列）：

1. **旧人工 Deep（`manual.v1`）在 v2 读取接口下的形状**：`evidenceRefs` 没有 `supportType`，前端不能按 v2 渲染时崩溃。
2. **`rejected` 中可能含模型编造内容**：确认只进评测导出，不进默认读取，也不进 Content Card。
3. **`input_manifest` 与 L1 请求表同名列**：读取/写入时不要交叉误用（L1 表存整个 Packet，`content_understanding.rs:219-220`）。
4. **Skill 包导入**：`content-deep-research` 的目录名、NFC、无 `\r`、无 `scripts` 段需有一条负例测试（`skill_package.rs:241-274` 已有范式，补目录名校验即可）。

## 14. PR 拆分与顺序

共享装配文件（`scripts/local-runtime.sh`、`apps/api/src/local_web/full_schema_fixture.rs`、`scripts/ci-select.mjs`）**只由 PR-1 修改**；后续 PR 依赖它。

| PR | 范围 | 依赖 | 验收命令 |
|---|---|---|---|
| **PR-1 迁移与注册** | 新迁移（4.1 全部）+ 三处装配 + `ai_failure_code` + `skill_allowed` + `requires_skill`（按 Mog 决定）+ prompt v2 + fixture 摘要测试 + `ci-select`（如需要） | 无 | `node scripts/check-document-governance.mjs`；`bash scripts/ci-preflight.sh origin/main`；隔离库迁移与摘要测试（`bash scripts/test-research-dossier-postgres.sh --ai-foundation` 或对应新建脚本） |
| **PR-2 请求 v2 与材料清单** | `deep_manifest.rs`（新文件）、`generate_deep.rs` 的模式/子问题/Core 版本/manifest/指纹/去重 | PR-1 | `cargo test -p linggan-intelligence --lib`；`bash scripts/test-research-dossier-postgres.sh --ai-service`（含 `content_understanding_executor_postgres`）；`bash scripts/ci-preflight.sh origin/main` |
| **PR-3 校验与执行器 v2** | `deep_grounding.rs` 的 payload v2 / accept v2 / 逐条判定 / 整次判定；执行器传图、High 强制、发布前指纹与 Core 版本重算；Skill 绑定后的真实准入 | PR-1、PR-2 | 同上 + `cargo test -p linggan-intelligence --test content_understanding_executor_postgres`；`bash scripts/test-research-dossier-postgres.sh --ai-service`（执行器隔离链路）；`bash scripts/test-research-dossier-postgres.sh --dossier-ui`（`content_understanding_real_browser_chain`，High/Skill 归属的真实准入与 HTTP 链路） |
| **PR-4 读取与前端** | `analysis.deepAnalyses[]`、`materials` 投影、`/deep/modes`、Core 版本徽标、前端类型与展示 | PR-1（数据形状）、PR-3（v2 记录） | `npm --prefix apps/web run lint` + 单元/组件测试；`bash scripts/test-research-dossier-postgres.sh --dossier-ui`（含浏览器链路）；`bash scripts/ci-preflight.sh origin/main` |

合并纪律：每个 PR 必须先跑预检与受影响 proof；PR-1 的迁移编号在合入时取（当前 `0222` 起）；不合并任何会改 `linggan_ai_task_setting`、Skill 绑定或 prompt 激活的迁移。

## 15. 需要 Mog 决定的问题

1. **`requires_skill` 取值。** 建议 `true`（fail-closed，缺 Skill 不派发）；代价是迁移后到激活前 L2 完全不可用，且现有隔离浏览器测试要改。若希望保留“无 Skill 也能跑 prompt v2”的过渡，则取 `false`，但那等于方法正文仍留在 prompt 里，与本版 C01 冲突。
2. **M1 的 `first_version` 措辞。** 当前 schema 已不支持（`0169`），首个版本只能 `manual` 激活。建议把 M1/第 7.2 节改为“首个版本以 `manual` 激活，`reason` 引用评测报告”，这属于修改已确认决策的文字，需你确认。
3. **Core 版本与 Deep 的展示口径。** 建议：读取返回全部 `deepAnalyses[]`，每条标 `coreBasis`；修订出现后，旧 Deep 仍显示但标“基于原始 Core”。如果你希望“修订后旧 Deep 直接隐藏”，则第 3.2 节的读取改动可以更小，但与 M6“记录所用版本”的目标不一致。
4. **L2 High 强制的失败语义。** 现在是 `defer`（无限等待）。建议保留 defer（配置可改），但要求在 AI 设置里显示原因；是否改成直接失败（`skip_unattempted`）需要你定。
5. **1/3 阈值的边界写法。** 规范写“超过 1/3 扣下”。建议实现 `rejected * 3 > total` 才扣，即 3 条拒 1 条仍发布。请确认这就是你要的边界（另一种读法是“≥1/3 即扣”，会改变首个评测结果）。
6. **被拒发现的保留范围与期限。** 规范只写“入库，仅用于评测”。它们可能含模型编造内容和原文摘录；建议保留原文摘录、限制访问、随评测包定期清理。需要你确认是否接受“原文级摘录进入数据库”。
7. **图片上限。** 规范写 9 张，但 adapter 没有硬上限，只有每张 1024 token 的预扣与 8 MB 单图限制。建议把 9 张写进 manifest 生成常量，并在读取接口的 `modes` 里公开这个上限；请确认 9 是产品上限而不是“与现有物化对齐”的经验值。
8. **Skill 正文 token 实测责任人。** 规范 9.5 把“3–4k”当估算。建议在首轮评测前用一次合成材料调用记录实测（不发送真实作品），由谁执行需你指定。

## 16. 未做与未证明

- 未连接运行库：L2 记录数、任务开关、当前模型配置与能力声明均**未核对**（第 2.2-1、8 节）。
- 未发起任何模型调用，未发送真实作品文字/图片/视频，未执行迁移（含本地），未开启 L2，未切换配置/prompt/Skill。
- 未评审附录 A 的 Skill 正文内容（由你评审）；只核对了字段名、枚举值与第 5 节合同的一致性。
- 未修改 V0.3 正文；发现的问题都写在本报告，建议文字见各节。
- 第 1.1、1.2 节（含 M1–M6）未重新论证。

## 17. 2026-11-16 补记（原文不改，按核实结果更正）

本节追加在报告末尾，正文保留当时（`main@c57e16b1`）的审查记录。以下四项是对原文的事实更正与状态更新。

### 17.1 迁移的 CI 路由：只选 `ai`，不会“选全部 proof”

**原文第 14 节与 K2 说“新迁移属于未知路径，会选全部 proof”，这是错的。** `scripts/ci-select.mjs:14` 把 `database/migrations/**` 明确归入 `ai` 组；`web` 组只单独列了 `0195`、`0196`、`0198`、`0199` 四个迁移（`scripts/ci-select.mjs:71-74` 一带），`web_real_chain` 没有迁移路径。因此只改一个新迁移文件时，Delivery 只会选中 `ai`，`web` 与 `web_real_chain` 不会运行。

**对 PR-1 的影响（二选一，写进 PR 正文）：**

1. 更新 `scripts/ci-select.mjs`：把新的 L2 迁移路径加入 `web`（读取接口与前端依赖它的列），必要时也加入 `web_real_chain`；同步更新 `scripts/ci-select.test.mjs` 的正反例。
2. 不改选择器：在 PR-1 正文里写明“本 PR 只自动跑 `ai`；需人工补跑 `web` 与 `web_real_chain`”，并在合并前实际补跑。

不要再用“未知路径会选全部”来解释覆盖。

### 17.2 `content_understanding_executor_postgres` 是 `#[ignore]` 测试

**原文第 12、13、14 节给的 `cargo test -p linggan-intelligence --test content_understanding_executor_postgres` 不会真正运行任何用例。** 该文件唯一的入口带有 `#[ignore = "owned PostgreSQL + AI_FOUNDATION_PROOF_NODE + owned LINGGAN_LOCAL_MEDIA_ROOT; no real models"]`（`crates/intelligence/tests/content_understanding_executor_postgres.rs:43`）。

- 直接跑要加 `-- --ignored`，并准备隔离 PostgreSQL、`AI_FOUNDATION_PROOF_NODE`、`LINGGAN_LOCAL_MEDIA_ROOT`。
- 仓库现有入口是 `bash scripts/test-research-dossier-postgres.sh --ai-service`（`scripts/test-research-dossier-postgres.sh:184`），它已经带 `--ignored`，优先用它。
- 浏览器链路仍只用 `--dossier-ui`（`scripts/test-research-dossier-postgres.sh:232-243`），这一点原文第 17.1 轮修正后已正确。

### 17.3 第 6 节与第 5 节的画面文字部分作废

- **第 6 节（位置与顺序）整节作废。** #594 已删除 `order`、`displayOrder`、`orderState`、`order_unsupported`、`order_ambiguous` 与第 5.4 节的顺序证明；规范现在只要求“摘录逐字、完整正文随结果保存，评审直接看先后”。
- **第 5 节中“画面文字时间”一行作废。** #596 已下线视频画面文字（[DEC-0013](../decisions/0013-retire-video-frame-text.md)）；`materialKind` 不再包含 `frame_text`，L2 视频只用口播转写及其时间。
- 第 5 节其余字段（正文合并、图片顺序/用途/SHA256 从 `inventory` 取、封面区分、重复图）仍有效；其中“图片展示顺序”仅用于请求发送与展示，不再参与任何顺序校验。
- 冲突清单 K9、K10、K12 随之作废；K9 的“图片 id 与发送顺序一致”保留为发送约定。

### 17.4 第 15 节八问已被 Mog 全部定案

2026-11-16 的八项决定（D1–D8，已写入规范第 1.2.1 节）与原文第 15 节一一对应：

| 原问题 | 决定 |
|---|---|
| 1 `requires_skill` | `true`；无 Skill 不派发，返回 `skill_not_configured` |
| 2 M1 措辞 | 首个版本以 `manual` 激活，`reason` 引用评测报告 |
| 3 Core 版本与 Deep 显示 | 全部 `deepAnalyses[]` + `coreBasis`；旧 Deep 照常显示；`publish_reviewed_core` 补 `lock_core` |
| 4 High 失败语义 | 保持 `defer`，AI 设置显示原因 |
| 5 1/3 边界 | `rejected * 3 > total` 才扣下 |
| 6 被拒发现保留 | 连同原文摘录写入 `deep.rejected`；读取不返回，仅评测导出 |
| 7 图片上限 | 9 张，manifest 常量，`/deep/modes` 公开 |
| 8 Skill token 实测 | 准备合成材料脚本但不运行，调用由 Mog 另行授权 |

另：本次补记与后续实施都以 `main@a343ba8f` 为准；原文的 `文件:行` 可能在 #594/#596 后漂移，实施前必须重新核对。
