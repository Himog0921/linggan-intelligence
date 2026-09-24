# PAGE-DOMAIN-MANAGEMENT-001 · 领域管理

> 状态: 权威当前
> 最后核对: 2026-09-24
> 适用范围: Collection 中正式 Domain 的查看、编辑、暂停/恢复与 Observation Target 关系管理
> 事实来源: Mog 2026-09-23 的领域平权决定、DEC-0008、DOMAIN-UNIFICATION-001 与 LIDS
> 冲突时以谁为准: 用户最新决定、AGENTS.md、DEC-0008、真实 API/schema/代码；本页不代表运行能力已接通
> 页面层级与模式: L1 Operations；Collection Control（现有 Collection 轨道与受控表单）
> 当前实现证据: SSR 路由与关系/配置写入已实现；2026-09-24 在隔离 PostgreSQL 与浏览器中验证配置、暂停/恢复、跨领域角色及冲突反馈；1440×1000 与 390×844 目视检查和按钮键盘焦点通过。DOM 横向溢出精确测量、数据库不可读浏览器态、真实媒体 lane、部署与 Mog 验收仍未验证

## 1. 页面责任

领域管理回答“系统正式研究哪些领域、每个领域观察什么，以及该领域是否接受新工作”。它管理研究上下文和 Target 用途，不发起搜索、平台访问或采集。

- 页面路由：`/collection/domains`。
- Collection rail 保留现有五面顺序，领域管理作为第六个子面追加在末尾。
- 三秒答案：系统有哪些正式研究领域，每个领域的观察目标和材料用途是什么，领域是否接受新工作。
- 五秒主动作：创建/编辑/暂停领域，关联/解除目标，切换目标用途并跳转到目标或材料读取页。
- 可查看 active 与 paused Domain；没有 Domain 时展示真实空态。
- 不支持删除 Domain，不提供能力开关、Prompt、权限、模板、颜色、图标或排序设置。
- 创建 Domain 只建立配置，不触发平台访问。

## 2. Domain 信息

列表及详情可以表达：

| 信息 | 允许表达 | 事实边界 |
|---|---|---|
| 名称、说明、研究目标 | 当前 Domain 配置 | 名称不是采集关键词；说明不自动形成 Topic |
| 状态 | active / paused | paused 停止新增工作，不隐藏历史材料 |
| Target 总数 | 当前 Domain–Target 关系数 | 数据不可读时为未知，不显示 0 |
| primary / reference 数 | 按当前关系 role 统计 | 不是材料数量或采集覆盖 |
| 作品、评论数量 | 当前 Domain 合格用途下的 distinct material 计数 | 只计有合格材料用途者；失败或缺失来源不能当作 0 |
| 最近接纳观察 | 最近的合格接纳时间与来源线索 | 不是平台最新内容时间或采集健康结论 |

详情按来源事实分 lane 显示发现、详情、评论、回复、媒体槽位、媒体字节、OCR、ASR。每一 lane 使用其自身真实状态；未知、未请求、部分、失败、仅远程候选和已本地化材料不得压成单一完整度百分比。

## 3. Target 用途

同一 Observation Target 可以同时出现在多个 Domain 中，并在每个 Domain 独立设置 `primary` 或 `reference`。页面支持关联、解除关联和变更 role；操作只变更当前配置，不复制 Target / Content，也不改写已冻结的 Request、WorkOrder、Package 或材料用途。

未关联任何 Domain 的 Target 可以留在候选列表，但不能排活。新关系只能关联 active Domain；paused Domain 保留当前关系与历史，不接受新的关系和采集工作。

## 4. Role 与材料可用性

| 行为 | primary | reference |
|---|---|---|
| 发现、详情、评论、回复、媒体与派生 lane | 同等基础能力 | 同等基础能力 |
| Corpus 与 Inspector | 默认研究材料 | 可完整查看并明确标记为参照 |
| Comment Study 默认输入 | 纳入 | 排除 |
| Comment Study 人工选择 | 可用 | 可显式选择；Run 与结果保留 role |
| 自动结论与默认统计 | 默认纳入 | 默认排除 |

具体 WorkOrder 范围决定采集哪些 lane。role 本身不授予额外平台访问、媒体处理或模型调用权限。

## 5. 状态与真实后果

| 状态 | 真实后果 | 页面必须说明 |
|---|---|---|
| active | 可建立 Target 关系、提交新采集 Request 和新研究策略/Run | 只说明可申请，不表示已排活或已采集 |
| paused | 不产生新的 Request/WorkOrder claim/Comment Study Policy/Run | 历史材料与既有结果可读；已 claim Attempt 有界完成 |
| 保存配置 | 写入名称、说明或研究目标 | 不自动搜索、采集或改变历史用途 |
| 关联 Target / 修改 role | 改变今后的当前关系 | 已准入用途和历史材料不被重写 |

同一 WorkOrder 冻结多个 Domain 用途时，任一用途 Domain paused，未 claim 的整张 WorkOrder 暂缓。不能只隐藏 paused Domain 的页面来暗示后台会继续工作。

## 6. 空态、失败态与呈现

- 新建空 Domain：说明尚无 Target 或材料；各 lane 可显示 `NOT_OBSERVED`，不推导来源世界为空。
- 关系为空：显示可关联的正式 Target；不自动分配或排活。
- 统计暂不可读：保留 `UNKNOWN` 与读取错误，不补 0。
- partial / failed lane：逐 lane 显示已观察状态和来源回执。
- media remote-only：只显示远程候选状态，不把 URL 当成本地资源。
- reference：列表、Corpus 与研究选择处持续保留“参照”标签。
- POST 冲突或数据库不可用：保留表单上下文，说明未写入；不得显示成功反馈。
- 页面在 1440 CSS px 桌面视口验收；不得新增横向溢出。交互遵守 LIDS 键盘、焦点和中文优先规范。

## 7. 验收 ID

- `DM-01` 平级 Domain：列表能区分 active/paused 且不设 home/外部身份。
- `DM-02` 配置写入：create/edit/pause/resume 只改配置，不创建平台工作。
- `DM-03` 多 Domain Target：一个 Target 可在多个 Domain 使用不同 role。
- `DM-04` Role：reference 与 primary 材料能力相同，研究默认范围不同。
- `DM-05` 暂停：历史材料可读；新请求、claim 与新研究写入被拒；在途 Attempt 有界结束。
- `DM-06` 状态诚实：UNKNOWN、partial、failed、remote-only、localized 分 lane 表达。
- `DM-07` 失败：冲突和存储不可用不伪装成功。
