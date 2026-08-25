# PLUGIN-RETROFIT-LOCAL-TRUSTED-001 · 本机可信浏览器回传

> 状态: 活跃计划
> 最后核对: 2026-08-25
> 适用范围: GitHub Issue #43 的 Linggan 自有浏览器插件与本机 loopback 的最小回传闭环
> 事实来源: 当前代码、`database/migrations/0003_local_trusted_producer.sql`、自动测试和 PostgreSQL proof
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

部分结果按既有 Discovery contract 接纳：已获得且合格的可见卡片仍可保存；Coverage 说明
此次观察缺口，只限制之后 Claim 的解释资格，不能连坐丢弃每一张已取得卡片；未知不补零。

## 实现边界

### 已实现

- 独立的 Manifest V3 `producer_instance_id`、`task_id`、`attempt_id`、`submission_id`；
- 浏览器独立 Dexie outbox，先持久化、后分批 delivery；超时与 service worker 重启后以同一
  submission identity 重试；页面侧返回 `queued / pending`，不等待网络；
- Linggan loopback-only API：创建 manual task、开始 attempt、提交 package；
- `0003_local_trusted_producer` 仅追加任务、attempt、delivery receipt 事实，并复用既有
  immutable Discovery admission；
- 真实隔离 PostgreSQL proof：首次接纳、timeout 后同一 submission replay、部分 Coverage 卡片
  保留、API receipt 与运行时合同负例。

### 明确不做

- 不访问真实 XHS/抖音、Cookie、账号、页面滚动、详情、评论、作者历史、批量自动采集；
- 不接 scheduler、工位、授权、旧内容工作台 endpoint/fallback 或旧写回；
- 不处理媒体字节、封面本地副本、OCR、ASR、AI、Topic、Claim、研究或页面产品动作；
- 不把浏览器 outbox 当作 Linggan Evidence、Corpus 或唯一事实来源。

## 验收与仍未证明的事实

自动和 PostgreSQL proof 只能证明合成输入的合同、持久 outbox、receipt 幂等与部分 Coverage
边界。它们不证明浏览器已安装、真实插件 UI 已经触发 package、真实平台页面可读、真实素材
已被接纳、Evidence Library 已出现真实数据，或媒体/研究链工作。上述每一项必须由下一张
经过授权的真实 Canary 单独验证。
