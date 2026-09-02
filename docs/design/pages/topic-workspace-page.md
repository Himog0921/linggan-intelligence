# PAGE-TOPIC-WORKSPACE-001 · 真实 Topic 工作区

> 状态: 权威当前
> 运行时状态: L2 本机真实读模型；Topic 仍为 `PROVISIONAL`
> 最后核对: 2026-08-31
> 适用范围: Issue #112 `/topics/{canonical_key}` 的产品任务、状态、布局、交互与验收边界
> 事实来源: TOPIC-WORKSPACE-REAL-001、Work Resource Read、LIDS、用户确认的视觉方向与当前实现
> 冲突时以谁为准: 用户最新确认、真实 API/数据库状态、Topic 合同、LIDS 与安全边界

## 1. 核心任务与三秒答案

- 核心任务：研究者核对一个暂定 Topic 的精确定义、人工材料裁定、挑战/边界和来源限制，并回到每条 Work Resource 检查依据。
- 三秒答案：这是第几版暂定定义、由谁/如何裁定、冻结包包含多少支持/挑战/边界材料、来源边界是什么。
- 五秒动作：按角色筛选材料，选择一条查看裁定理由与 Work Resource 当前读模型，打开同源详情 JSON。
- 非目标：正式发布 Topic、自动分类、趋势、总体统计、采集、Claim、Agent 运行、行动。

## 2. 强度与视觉方向

| 字段 | 固定值 |
|---|---|
| 强度 | L2 · Research / Analysis |
| 视觉论点 | 白色连续研究台；煤黑结构承载精确关系；橙红只标当前选择和候选边界 |
| Pattern | Definition Header + Classification Lens + Frozen Material List + Work Resource Inspector + Source Boundary |
| 字体 | 中文 Sans 主语义；Mono 只作 ID、版本和技术键 |
| 动效 | 100–160ms 选择/按压反馈；Reduced Motion 立即切换 |
| 禁止 | 装饰图表、趋势箭头、伪 KPI、正式成功 Tag、暗色终端、渐变营销 Hero |

## 3. 信息架构

```text
Shared Header / Topic Map active / provisional boundary
Local rail: Definition → Materials → Source Boundary
Definition + Version                    Run / Pack / Read state
Classification role lens | Frozen Work Resource list | Current material inspector
Source Boundary                        Explicit non-claims
```

页面以定义为主，不以材料数量为 Hero；材料列表是可核验集合，不是推荐流。右栏只解释当前选择，不聚合出新的判断。

## 4. 状态矩阵

| 状态 | 页面表达 | 不得表达 |
|---|---|---|
| API 未接通/读失败 | 当前未读取 Topic；没有显示缓存或合成数据 | 空 Topic、零材料、采集失败 |
| Topic 不存在 | 明确 not found 技术原因 | 自动创建或回退静态 reference |
| 可读 | 暂定版本、人工 run、exact pack、逐条 Work Resource | 正式 Topic、正确率、总体代表性 |
| 某角色 0 条 | 当前裁定视角没有材料 | 来源世界中不存在该类材料 |
| Work Resource 不可履行 | 整体读失败/冲突 | 静默缩小 pack 或用旧字段补齐 |
| 字段 Unknown | 显示未知或共享 Work Resource limitation | 0、空字符串、推断作者/时间 |

## 5. 交互与无障碍

- 角色按钮使用 `aria-pressed`，四种视角共享同一精确材料集合。
- 材料行使用 `role=option + aria-selected`；当前实现支持点击/Tab/Enter 原生按钮语义。
- Focus 使用共享 2px signal outline；所有可达目标保持可见。
- 页面不自动播放、不用 hover 承载必要信息；≤1080px Inspector 下移，≤720px 转连续单栏。
- Source 文本进入 DOM 一律通过 `textContent`，不拼进 `innerHTML`。

## 6. 数据与动作

- 页面 GET：`/api/local/topic-workspaces/{canonical_key}`。
- 导入 POST 是 loopback 受控接口，不在页面暴露按钮。
- 页面只消费 API 的 `topic + materials[].classification + materials[].workResource + detailUrl`。
- Work Resource 的标题、作者、发布时间、lane、媒体与限制仍由共享读模型拥有。

## 7. 与静态 Reference 的关系

`PAGE-TOPIC-001` 及其 HTML 仍是 2026-08-21 的合成设计参考。当前页面没有把它“升级成真实”：新页面另立 PAGE ID、路由、合同和验收；旧 reference 的合成 sample、local intent 和非运行声明不得混入真实页。

## 8. 验收

1. 首屏无需互动即可看到“暂定主题/人工裁定/来源不得外推”。
2. API 不可用时保持“未读取”，不显示 reference sample 或假材料。
3. 可读时 Definition/Run/Pack IDs 和材料引用一一对应，角色筛选不重新查询或改变包。
4. 任一材料的显示字段都可回到同源 Work Resource；Topic API 不返回 raw body/comment copy。
5. CSS 只消费现有 LIDS token；桌面与窄屏无横向溢出，Reduced Motion 有效。
