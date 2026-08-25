# PLUGIN-REHOME-001 · 旧插件迁入与运行边界映射

> 状态: Draft migration map（REAL-XHS-SEARCH-DISCOVERY-001 已接入 current-surface discovery）
> 最后核对: 2026-08-25
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
| 页面注入控制 | XHS 注入 UI 保留；唯一激活动作是用户在 `ADHD` / “综合”搜索页点击当前前 20 条 discovery | 原来的页面入口和明确回执；其余动作仍 pending | 不滚动、不打开详情、不读 Cookie/账号或隐藏状态 |
| XHS / 抖音 collector source | XHS legacy 搜索面语义被收敛为 `xhsVisibleSearchReader`；其余 collector source 仅保留参考 | 首批动作沿用稳定链接、视觉顺序、去重经验 | 抖音和详情/评论/媒体 collector 仍未接通或验证 |
| 本机 Dexie / 恢复逻辑 | 单独 LOCAL_TRUSTED outbox 保存用户手动的真实 Discovery submission；其余 legacy source 仍不进入 active runtime | 当前面卡片先本地持久化，网络超时或 service worker 重启后以同一 submission id 重试 | outbox 不是 Linggan Evidence / 数据库真相；scheduler 为 `NOT_CONNECTED` |
| 旧授权、轮询、lease、工位派发 | 旧 source 仅作历史保留；Webpack 不以旧 background 为入口 | 不再出现旧工作台授权成功或工位就绪承诺 | 当前 active service worker 不调用这些链路 |
| 旧数据同步 / fallback | 已从当前 Dashboard 动作中切断 | “提交到 Linggan（待接通）”只显示原因，不会提交 | 不存在对旧工作台的兼容提交 |
| cookies、媒体下载、定时、网络规则、通知权限、抖音/CDN host access | 从 Manifest 移除 | 只请求本机 loopback 与当前 XHS 页面所需权限 | 所有需要这些能力的未来接入必须重新评审 |
| 详情、评论、媒体、抖音、自动化 | 代码与 UI 位置保留，运行动作均等待 adapter | 点击会得到具体未接通说明 | 未访问详情、未下载媒体、未写入其他 Linggan 资产 |

## 新旧运行路径对照

```text
旧路径（本轮禁止）
Popup / injected control
  -> old background
  -> old authorization / station / lease / polling
  -> 内容工作台 endpoint / sync / fallback

当前路径（本轮实际）
Popup / Dashboard / injected control
  -> Linggan adapter boundary
  -> user-initiated current XHS search surface TaskSpec / Attempt / durable Submission outbox
  -> localhost receipt or retry; no old endpoint

页面采集动作（本轮仍未接通）
  -> explicit PENDING result
  -> no platform action, no media write

以后单独授权的路径（本轮未实现）
user action -> authorized Linggan adapter -> bounded work -> receipt / Coverage
```

## 发布前可验证项与未验证项

本卡的自动验证应证明：完整 source 与 UI entrypoints/assets 存在、可构建、可生成稳定
release ZIP，新的 `npm ci/build/package` 与已提交 ZIP SHA-256 一致，active content
bundle 不包含旧工作台 host/endpoint/运行时模块（lease、heartbeat、outbox、polling），
且 Manifest 不保留不应启用的旧权限。

自动验证不能证明：浏览器实际加载、真实 XHS 页面兼容、账号可用、真实采集、媒体
保存、本地封面、OCR/ASR、Linggan 数据库接纳、Evidence Library 展示或研究结论。
