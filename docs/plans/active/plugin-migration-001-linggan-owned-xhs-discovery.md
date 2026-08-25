# PLUGIN-MIGRATION-001 · 将成熟 XHS 发现能力适配为 Linggan 自有插件

> 状态: 活跃计划
> 最后核对: 2026-08-25
> 适用范围: Issue #37 的 Linggan 自有浏览器插件 source 迁移、受限的 XHS 搜索页 Discovery 与本机 ingress 提交
> 事实来源: Mog 于 2026-08-25 的最新确认、Issue #37、`LOCAL-001C0-DISCOVERY-BOUNDARY-V1`、当前 Linggan `origin/main`，以及只读迁移源 `Himog0921/linggan-boom@8a00cc1`
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、现行 Discovery 合同/实际代码/测试；迁移源只说明既有能力，不是 Linggan 运行时规格

## 用户可见目标

Linggan 拥有一份完整、可构建、可加载的浏览器插件源包。用户在小红书 `ADHD` 搜索结果页明确点击后，插件只读取当前搜索卡片面中实际存在、可稳定链接识别的前 20 张卡片，生成既有 `xhs.discovery.visible-card.v1` 并仅提交到 `http://localhost:3000/api/local/discovery-packages`。它显示本机接纳、重放或未接纳回执与真实 `visibleCards`/`stoppedReason`。

## 迁移原则

这不是从零再造采集器，也不是把旧产品搬进 Linggan。迁移源是当前实际旧插件 `Himog0921/linggan-boom@8a00cc1` 的 XHS 搜索页成熟经验：

| 旧插件能力/来源 | Linggan 处置 | 本卡状态 |
|---|---|---|
| `src/platforms/xhs/noteCollector.js` 的 `discoverNotesFromDOM`、视觉顺序、`a.cover` + stable note id 过滤 | 适配为 Linggan `xhs-visible-search-adapter`；保留当前 surface、不滚动、去重和稳定链接原则 | **ACTIVE** |
| `docs/technical/XHS_FIELD_SURVEY.md` 5.1 的搜索页调研 | 写入 adapter 迁移注释和测试 fixture 的来源；不把调研文字当真实运行证明 | **SOURCE EVIDENCE** |
| Popup、MV3 service worker、用户动作 → 页面适配器 → 回执的成熟运行形态 | 保留形态，替换为本机固定 ingress、最小权限和 Linggan 状态文案 | **ACTIVE** |
| 旧内容工作台 API、任务轮询/lease、站点授权、IndexedDB/Dexie outbox、远程/线上 host fallback | 不迁入，不保留兼容分支或运行时字符串 | **NOT ADAPTED** |
| 详情、评论/回复、博主历史、下载、媒体、OCR/ASR、抖音、后台自动化、Cookie/账号访问 | 不迁入当前 release；以后逐项单独规格、权限和 Canary | **DISABLED / NOT ADAPTED** |

## 冻结范围

**允许**：

- 插件内完整 source/build/release、Popup、MV3 service worker、XHS 可见搜索卡片 adapter、模拟 loopback 提交与相关测试；
- 以 `activeTab + scripting` 为用户手势临时权限，和固定 `http://localhost:3000/*` host permission；
- 所有当前可见/存在但字段缺失的结果保持字段缺失；缺稳定内容 id 的卡片跳过，绝不猜造 id。

**禁止**：

- 任何旧内容工作台、旧插件 dist、旧数据库、旧 endpoint、旧站点、队列、授权或回退运行依赖；
- persistent XHS host permission、content script、Cookie、账号、downloads、alarms、IndexedDB outbox、网络/隐藏状态读取；
- 页面滚动、详情、评论、作者、媒体字节、OCR/ASR、AI、后台/定时采集；
- Rust/API/数据库合同变更，真实浏览器/XHS/账号或真实材料访问。

## 执行与验证

1. 将旧插件的搜索页已验证 adapter 规则适配到 Linggan 自有模块，连同一个只读 source-inventory，明确哪些能力未适配。
   **验收：** adapter 输入的可见 DOM fixture 得到稳定的卡片顺序；不合格卡不产生伪造记录。
2. 用 service worker 把一次明确点击绑定为 `activeTab + scripting` 页面读取、既有 JS discovery contract 和固定 loopback submit。
   **验收：** 错页、关键词/排序不满足、selector/稳定 id 不足、localhost 无法接纳均返回可见且不提交的状态。
3. 更新 popup、构建、可安装 release、runbook、索引和当月记录。
   **验收：** clean build/release、静态 no-workbench/no-forbidden capability audit、adapter/contract/mock-submit tests、治理检查通过。

## 停止与未证明边界

- 如果 `activeTab + scripting` 不能在用户明确点击时读取当前 XHS 搜索页，停止，不能扩大为 persistent XHS host permission。
- 如果页面无法以可见 selected state 确认“综合排序”，停止提交；不能把 URL、默认值或旧系统状态猜成当前排序。
- Mock loopback 提交仅证明插件可发出正确请求和处理回执；不证明 Linggan 本机持久数据库已配置、浏览器已加载、真实 XHS 字段仍有效、真实 platform collection 成功或 Evidence Library 已展示真实材料。

## 完成声明格式

PR 必须分别列出迁入来源、启用能力、隔离/未适配能力、构建和模拟证据、release/安装路径，以及上述未证明边界。实现者不合并、不关闭 Issue。
