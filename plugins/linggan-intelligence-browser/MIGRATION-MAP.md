# PLUGIN-REHOME-001 · 旧插件迁入与运行边界映射

> 状态: 当前迁移与自动执行边界
> 最后核对: 2026-08-31
> 适用范围: `linggan-boom@8a00cc1` / `v2.0.91` 到 Linggan 自有浏览器包的第一条垂直迁入边界
> 事实来源: `plugins/linggan-intelligence-browser/` 当前 source、Manifest、Webpack active entries 和 isolation check
> 冲突时以谁为准: 当前可构建 source 和实际运行证明；旧插件只说明迁入来源，不是 Linggan 运行规格

## 迁入来源与所有权

| 项目 | 事实 |
|---|---|
| 来源 | `Himog0921/linggan-boom@8a00cc1`，tag `v2.0.91` |
| 新所有者 | 本仓库 `plugins/linggan-intelligence-browser/` 的独立 source、lockfile、build 和 release |
| 不允许的关系 | 不链接、导入、调用、发布或回退到旧内容工作台/旧插件仓库 |
| 活跃后台入口 | `src/linggan/background.js`，由 `webpack.config.cjs` 唯一指定 |

## 逐层处置

| 原有能力 | 本轮处置 | 用户现在看到什么 | 当前事实边界 |
|---|---|---|---|
| Popup / Dashboard | 已原样迁入其主要布局、主题、品牌资产、数据页和操作位置 | 熟悉的灵感爆爆爆操作界面，Linggan 本机状态区替代旧工作台连接区 | 界面可构建；没有真实数据证明 |
| 页面注入控制 | 已保留 XHS / 抖音页面识别、按钮位置和提示体验 | 原来的页面控制入口仍在 | 未接通动作显示 Linggan 未接通，不是采集成功 |
| XHS / 抖音 collector source | 完整保留在 `src/platforms/` 和关联 source | 后续可在同一源码包上适配 | 不能在本轮视为已授权、已接通或已验证 |
| 本机 Dexie / 恢复逻辑 | Linggan 专属 outbox 保存待交付 Submission 与媒体上传；service worker 重启后继续同一不可变标识 | 人工与 scheduled Package 均可恢复投递 | outbox 不是 Evidence / 数据库真相；调度事实仍在 Rust/PostgreSQL |
| 旧授权、轮询、lease、工位派发 | 旧 source 仅作历史保留；Webpack 不以旧 background 为入口 | 不再出现旧工作台授权成功或工位就绪承诺 | 当前 active service worker 不调用这些链路 |
| 旧数据同步 / fallback | 已从当前 Dashboard 动作中切断 | “提交到 Linggan（待接通）”只显示原因，不会提交 | 不存在对旧工作台的兼容提交 |
| cookies、媒体下载、定时、网络规则、通知权限 | 从 Manifest 移除 | 不会请求这些旧能力的浏览器权限 | 所有需要这些能力的未来接入必须重新评审 |
| 详情、评论、媒体、批量、抖音、自动化 | 代码与 UI 位置保留，运行动作均等待 adapter | 点击会得到具体未接通说明 | 未访问平台、未下载媒体、未写入 Linggan |

## Issue #78 / #80 的当前执行边界（覆盖上表中旧的 XHS 静态入口描述）

| XHS 能力 | 当前处置 | 不构成的承诺 |
|---|---|---|
| 搜索/博主页发现 | 以有限页面加载、稳定内容去重和目标数为执行边界；记录实际数、轮次、页面加载事实与停止原因 | 不调用旧 API snapshot，不宣称平台全量或调度已接通；不以随机化或规避机制替代页面结束判断 |
| 标准详情 | “采集当前笔记到 Linggan”交付详情、媒体槽位观察和最多 30 条评论，三条 lane 分别显示接纳状态 | 单一 lane 的接纳不代表全部接纳；该入口不打开媒体选择窗口、不下载媒体原件 |
| 人工媒体下载 | “人工采集并下载媒体”保留原有媒体选择窗口和本地下载服务 | 只在人工显式操作后下载；不把人工下载结果写成 Linggan 自动接纳或媒体理解完成 |
| 单篇/批量评论 | 深采可明确设定上限或“公开自然结束”，批量逐篇 checkpoint、空态成功、失败隔离 | 不做内容分析、不绕过验证、不中断时不生成真实 Evidence |
| 本机恢复与媒体 | 使用独立 `LingganIntelligenceBrowserLocalStaging`；媒体仅允许受限 HTTPS 平台域名 | 不复用 `LingganBoomDB`，不允许任意页面消息/本机 URL 作为下载来源 |
| 发现封面自动物化（0.7.0） | 服务端从已接纳 discovery finding 建立 cover slot 与最多三代取得工作；插件自动领取、哈希、分块上传 | 不自动展开详情/评论/OCR/ASR；工作状态不是 Evidence，来源 URL 也不是长期资产 |
| 固定作品深化（0.8.0） | 上游把明确作品集合冻结到 WorkOrder；插件自动顺序执行详情、媒体槽位、评论与回复 | 不对所有发现结果无界 fan-out；普通巡检仍只发现变化 |
| 多样媒体卡槽（0.8.0） | 图片、封面、视频、实况图片进入同一媒体合同；实况 still/motion 分组件候选与有界取得工作 | URL 不是长期资产；一个组件成功不冒充整个实况卡槽完整 |
| 本机派生（0.8.0） | Rust worker 对已物化字节运行 thumbnail/OCR/audio/ASR/frame OCR，空文本写 KNOWN_EMPTY | 插件不承担 OCR/ASR；处理工作不冒充 Evidence 或来源事实 |
| 无人值守页面就绪（0.8.1） | 先订阅 tab 后读取当前状态，既承接未来 complete 事件，也接纳已经 complete 的页面 | 只修复页面就绪竞态；不改任务身份、租约、Attempt 或接纳规则 |
| XHS 签名详情定位（0.8.2） | 服务端从最新已接纳发现记录派发带 `xsec_token` 的短期来源链接；插件校验作品身份后转换为 `/discovery/item/{id}` | token 不进入 TaskSpec、作品身份或 Evidence；缺失时不执行，不回退裸 `/explore` |
| 同工位安装承接（0.8.2） | 新安装取代旧安装时承接其有效租约内尚未完成的同一 Task | 不重建 Task、不重跑完成步骤；已被取代安装不能再改变执行状态 |
| XHS 最终页面稳定（0.8.3） | 首次 `complete` 后继续观察 URL/加载变化；保持 1.5 秒稳定且最终 content script 回应同一 URL 才下发采集动作 | 总等待最多 20 秒；不新建 Task/Attempt，不把页面可响应冒充 Package 或 Receipt |
| XHS 部分详情与同页执行（0.8.4） | 结构化详情已取得内容/作者/媒体时允许互动字段部分未知；首个详情 Task 按服务端已批准范围一次读取详情、媒体候选与有界评论树，后续单能力 Task 复用持久缓存 | 不以未知补 0；不合并 Task/Attempt/Package/Receipt；缓存不授权新 lane、不跨 Lease，缺失或过期时回退原 lane 执行 |
| XHS SSR 详情读取（0.8.5） | 全局 `__INITIAL_STATE__` 被页面水合删除时，从原始页面脚本安全解析序列化 `noteDetailMap`，供详情就绪、完整度和正式采集共用 | 不执行页面脚本、不放宽作品身份校验、不把 SSR 内容冒充 API 或 DOM 观察；无 SSR 时仍走现有运行态/DOM 路径 |
| 评论回执与当前投影（0.8.6） | 标准 30 条窗口与深采分开；短采保留，重试从评论入口重新遍历；服务端按稳定评论身份形成当前投影 | 不跨 Attempt 累加数量冒充完整，不从旧评论游标续采，不修改历史 Package |
| 真实评论树接纳修复（0.8.7） | 顶层自指 root 与回复多关系字段在出包前规范到评论/回复唯一合同；自然结束深采使用无数值配额的有界停止条件 | 不从结果反推任务配额，不放宽服务端关系门闸，不改 scheduled TaskSpec |
| 克制评论执行与暂停交付（0.8.8） | API 分页、DOM 滚动、回复展开每轮三选一；最低冷却、页面稳定、暂停/停止检查共用；暂停提交真实 PARTIAL 评论树；进度区分已取得、页面公开数和请求上限 | 不伪装随机真人轨迹，不把 checkpoint 计数冒充评论材料，不跨 Attempt 累加完整性 |
| 媒体领取代际保护（0.8.8） | 领取后先安排恢复唤醒，再写可靠队列并执行；入队或执行异常按 workRef/claimGeneration 回报 | 不把来源 URL 当本地封面，不让人工媒体窗口消失，不把排队冒充物化完成 |

## 新旧运行路径对照

```text
旧路径（本轮禁止）
Popup / injected control
  -> old background
  -> old authorization / station / lease / polling
  -> 内容工作台 endpoint / sync / fallback

当前路径（0.8.8）
Popup / Dashboard / injected control
  -> Linggan adapter boundary
  -> scheduled 或 manual TaskSpec / Attempt / durable Submission outbox
  -> localhost receipt / retry / Evidence Material Projection

自动观察路径
Observation rule -> WorkOrder -> ordered single-capability steps
  -> first signed detail page read -> lease-scoped persistent page cache
  -> each claimed lane -> its own immutable Package -> Receipt
  -> local media bytes -> local processors -> Evidence Library
```

## 发布前可验证项与未验证项

本卡的自动验证应证明：完整 source 与 UI entrypoints/assets 存在、可构建、可生成稳定
release ZIP，新的 `npm ci/build/package` 与已提交 ZIP SHA-256 一致，active content
bundle 不包含旧工作台 host/endpoint/运行时模块（lease、heartbeat、outbox、polling），
且 Manifest 不保留不应启用的旧权限。

自动验证不能证明：浏览器实际加载、真实 XHS/抖音页面兼容、账号可用、真实采集、媒体
保存、本地封面、OCR/ASR、Linggan 数据库接纳、Evidence Library 展示或研究结论。
