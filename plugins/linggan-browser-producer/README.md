# Linggan Browser Producer

> 状态: 受限实现
> 版本: `0.2.0`
> 适用范围: `PLUGIN-MIGRATION-001` 的 Linggan 自有 Manifest V3 浏览器 Producer 包
> 事实来源: Issue #37、`PLUGIN-MIGRATION-001`、`LOCAL-001C0-DISCOVERY-BOUNDARY-V1` 与 [`docs/runbooks/linggan-browser-producer-local.md`](../../docs/runbooks/linggan-browser-producer-local.md)
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、版本化 Discovery 合同和真实加载/运行证据

这是 Linggan Intelligence 仓库内唯一获准的未来运行时 Browser Producer。它不依赖、导入、调用或回退到内容工作台的插件、服务、发布包、数据库或队列。

当前版本是独立、可构建的 Linggan 插件包。它复用了旧 `linggan-boom` 当前主分支中已被长期使用的 XHS 搜索页可见卡片发现规则，但没有携带旧产品的运行时依赖。首个启用能力只有：

1. 明确显示自身身份、版本和固定 Linggan loopback 目标；
2. 对 `http://localhost:3000/health` 检查本机 Discovery 接纳是否真正就绪；
3. 当且仅当用户在小红书 `ADHD` 搜索结果页、页面可见选中“综合”且主动点击时，以 `activeTab + scripting` 读取当前搜索卡片面中最多 20 张可稳定识别的卡片；
4. 把已有 `xhs.discovery.visible-card.v1` Discovery 合同提交到固定 `http://localhost:3000/api/local/discovery-packages`，并显示 Accepted、Replay、Not accepted 或 Not ready 回执。

它没有 persistent 小红书 host permission、content script、Cookie/账号访问、详情、评论、作者历史、媒体下载、OCR/ASR、旧任务轮询/lease、旧站点授权、IndexedDB outbox、后台自动化、旧内容工作台 endpoint 或回退。当前搜索页看到的远程封面只能作为 `coverCandidate` 的来源线索，绝不会直接用作 Linggan 页面展示地址。

## 已迁入与明确未适配

| 范围 | 处置 |
|---|---|
| XHS 搜索页 `.feeds-container section`、`a.cover`、稳定链接身份、视觉顺序、去重 | 已适配为 `src/xhs-visible-search-adapter.js`；不滚动、不打开详情、不读取隐藏页面状态 |
| Popup / MV3 service worker / 用户动作回执形态 | 已以 Linggan 的固定 loopback ingress 和最小权限重建 |
| 旧工作台、任务调度、站点授权、Dexie outbox、详情/评论/作者/媒体/抖音/自动化 | `DISABLED / NOT ADAPTED`；没有兼容运行路径 |

迁移来源的实际版本、剥离理由和未证明边界详见 [`PLUGIN-MIGRATION-001 计划`](../../docs/plans/active/plugin-migration-001-linggan-owned-xhs-discovery.md)。

## Build

```bash
cd plugins/linggan-browser-producer
npm run build
npm run check
```

构建产物：

- 临时、未提交的 `dist/`：浏览器加载目录；
- 已提交的 `releases/linggan-browser-producer-0.2.0.zip`：可安装包；
- 已提交的 `releases/release-manifest.json`：版本、ZIP SHA-256 与文件 SHA-256。

`npm run check` 会运行 adapter、合同和 mock loopback 提交测试，再从 clean source 重建临时包并比对当前 release manifest。它不加载浏览器、不调用 Linggan host，也不访问任何平台。

它仍然不证明浏览器已加载、本机数据库可接纳、真实小红书页面字段未漂移、真实 Discovery 成功或 Evidence Library 已展示真实材料。
