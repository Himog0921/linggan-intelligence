# PLUGIN-RETROFIT-LOCAL-TRUSTED-001 · 本机可信浏览器回传

> 状态: 已完成计划
> 最后核对: 2026-08-25
> 适用范围: GitHub Issue #43 的 Linggan 自有浏览器插件与本机 loopback 的最小回传闭环
> 事实来源: 已合入 `main` 的 PR #46（merge commit `123311c8dd98aa5c449b4ea508468a074a88db75`）、当前代码、`database/migrations/0003_local_trusted_producer.sql`、自动测试和 PostgreSQL proof
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、当前活跃 scope/合同、实际代码与可复现验证

## 用户可见目标

Linggan 自有浏览器插件保留灵感爆爆爆的已迁入界面和页面侧能力，但在本机测试阶段不再需要
旧内容工作台授权、登录、工位、租约、心跳、旧任务轮询或旧写回。一次**已经由页面侧得到的**
Discovery package 可以先可靠写入浏览器本地待交付队列，再非阻塞地提交给
`http://localhost:3000`；网络超时或浏览器后台重启不会把同一批材料重复写入 Linggan。

本卡不把“回传通了”说成“真实小红书已经采集”。

## 已冻结的最小合同

插件只理解扁平、可执行的 `TaskSpec`：

```text
contract_version / task_id / source / platform / page_type / target
capabilities_requested / maximum_quota / comment_limit / acquire_media
risk_policy / stop_conditions
```

本卡只接受一个固定 manual 形态：`xhs`、`search_results`、当前可见搜索面、最多 20 张卡片、
不请求评论或媒体。`scheduler` 明确返回 `NOT_CONNECTED`，不生成伪任务。

三类状态绝不合并：

```text
Execution: Attempt 已开始
Delivery: Submission 已排队 / 重试 / receipt acknowledged
Admission: 嵌套 Discovery package 被 accepted 或 immutable replay
```

每个 `attempt_id` 只能冻结一份 terminal package：相同 `submission_id` 与完全相同的
immutable payload 只返回原 receipt；同一 attempt 的新 submission 或不同 payload 明确冲突，
不会触发第二次 Discovery admission。需要继续采集时，必须创建新的 attempt。

部分结果按既有 Discovery contract 接纳：已获得且合格的可见卡片仍可保存；Coverage 说明
此次观察缺口，只限制之后 Claim 的解释资格，不能连坐丢弃每一张已取得卡片；未知不补零。

## 已合并的实现边界

### 已实现

- 独立的 Manifest V3 `producer_instance_id`、`task_id`、`attempt_id`、`submission_id`；
- 浏览器独立 Dexie outbox，先持久化、后分批 delivery；超时与 service worker 重启后以同一
  submission identity 重试；页面侧返回 `queued / pending`，不等待网络；
- Linggan loopback-only API：创建 manual task、开始 attempt、提交 package；
- `0003_local_trusted_producer` 仅追加任务、attempt、delivery receipt 事实，并复用既有
  immutable Discovery admission；
- 真实隔离 PostgreSQL proof：首次接纳、timeout 后同一 submission replay、部分 Coverage 卡片
  保留、attempt terminal conflict、API receipt 与运行时合同负例；
- 未读取 Linggan 数据时，插件统计返回 `not_connected / unknown` 与空值，界面显示“未连接”或
  “未知”，不把未知写成 0。

### 明确不做

- 不访问真实 XHS/抖音、Cookie、账号、页面滚动、详情、评论、作者历史、批量自动采集；
- 不接 scheduler、工位、授权、旧内容工作台 endpoint/fallback 或旧写回；
- 不处理媒体字节、封面本地副本、OCR、ASR、AI、Topic、Claim、研究或页面产品动作；
- 不把浏览器 outbox 当作 Linggan Evidence、Corpus 或唯一事实来源。

## 实际完成与合并后核对

- implementation head `96a60da4718ce5e40fb4911e34762d673746f204` 已由非实施者审查后通过
  [PR #46](https://github.com/Himog0921/linggan-intelligence/pull/46) 合并到 `main`；merge commit 为
  `123311c8dd98aa5c449b4ea508468a074a88db75`。
- 合并后 integration record 已核对：本地 `main` fast-forward 到上述 merge commit，
  implementation head 是 `origin/main` 的祖先，`main...origin/main` 无差异，
  `git diff --check origin/main` 与 `./scripts/check-project-governance.sh origin/main` 均通过。
- 本计划的技术完成层仅为：合成 manual Discovery 的 versioned TaskSpec、attempt terminal
  package、持久 outbox、loopback receipt、partial Coverage、replay/conflict 规则及其隔离
  PostgreSQL / API / 插件合同证明。该完成层不替代浏览器或平台链路证明。

## 验收与仍未证明的事实

自动和 PostgreSQL proof 只能证明合成输入的合同、持久 outbox、receipt 幂等与部分 Coverage
边界。它们不证明浏览器已安装、真实插件 UI 已经触发 package、真实平台页面可读、真实素材
已被接纳、Evidence Library 已出现真实数据，或媒体/研究链工作。上述每一项必须由下一张
经过授权的真实 Canary 单独验证。

## Issue 状态与后续

本计划的实现与合并后核对已经完成，因此从 `docs/plans/active/` 迁入本目录。GitHub Issue #43
仍保持 OPEN：本卡的文档 Draft PR 仍须由非实施者独立审查并合并；之后再重新核对 `main`、本计划、
当前状态和月度记录，并由非实施者按分层完成证据手工关闭 Issue。该治理步骤不代表尚未完成的真实
浏览器、平台、媒体或研究能力。
