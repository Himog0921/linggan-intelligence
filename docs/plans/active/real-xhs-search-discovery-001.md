# REAL-XHS-SEARCH-DISCOVERY-001 · 当前搜索页首批真实 Discovery 接入

> 状态: 活跃计划
> 最后核对: 2026-08-25
> GitHub Issue: #48
> 适用范围: Linggan 自有 `linggan-intelligence-browser v0.3.3` 的唯一已接通真实页面动作
> 事实来源: Mog 已确认的 REAL-XHS-SEARCH-DISCOVERY-001 范围、`data-contracts/local-001-discovery-evidence-boundary.md`、当前 source 与自动验证
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、`data-contracts/local-001-discovery-evidence-boundary.md`、当前代码与可复现验证

## 目标

用户已经打开、已登录小红书的 `ADHD` / “综合”搜索结果页后，明确点击旧插件原有搜索发现入口。插件只读取当时已经渲染在当前页的前 20 张可稳定识别卡片，经已有的 LOCAL_TRUSTED `TaskSpec → Attempt → durable outbox → loopback receipt` 交付。服务端只接纳 discovery 卡，并让符合 Evidence Library 发布窗口的已接纳卡片出现在本地页面。

## 本卡固定范围

1. 只在 `xiaohongshu.com/search_result?keyword=ADHD` 且页面可确认“综合”选中时允许动作；不合格时显示可理解原因，零提交。
2. 复用灵感爆爆爆的“已渲染 `section` + `a.cover` 稳定链接、视觉顺序、去重”的成熟搜索面语义；不导入旧 Workbench runtime。
3. 不滚动、不翻页、不打开详情、不读取 Cookie/账号、隐藏状态或网络接口；不推断平台总数。
4. 当前面最多检视 20 个卡片。每包永久记录 `visible / discovered / emitted / failed / notAttempted` 与真实停止原因；部分可用卡独立入库，Coverage 仅限制解释资格。
5. 只写入 discovery package/content identity/occurrence/Coverage；禁止详情、评论、作者档案、媒体、OCR/ASR、Observation、Topic、趋势、Claim。
6. Evidence Library 只展示服务端已接纳的 discovery 卡；封面必须持续显示 `MEDIA_NOT_ACQUIRED`，不使用小红书 CDN。
7. 其余旧 UI 动作继续是明确 pending，零副作用。插件只拥有小红书当前页面和 `localhost:3000` 所需权限；没有抖音/CDN/旧工作台权限。

## 验收与停止条件

| 层级 | 本卡要求 | 证据 |
|---|---|---|
| 合同 | 正例、缺 counter、counter 不一致、详情/媒体字段等负例安全失败 | Rust 合同测试 |
| 页面读取 | legacy 与替代卡片结构、排序、配额与不滚动规则可控证明 | DOM fixture 测试 |
| 交付 | 浏览器 local outbox 重启/超时保持同一 identity；4xx terminal 不无限重试 | 插件测试 |
| 接纳 | 隔离 PostgreSQL 证明 partial card 接纳、replay/conflict、Coverage 追加列 | PostgreSQL proof |
| 页面 | 合成已接纳卡能由 Evidence Library 投影显示，永不输出远程封面 URL | API/UI route proof |
| 发布 | v0.3.3 ZIP、release manifest、可重现打包与 runtime isolation | npm release checks |

本卡到 Draft PR 即停止：不安装、不访问真实 XHS、不读取真实账号/Cookie、不执行用户首次真实采集。真实链路与用户验收必须在独立审查、合并、安装新 ZIP 后由 Mog 明确手动触发。

## 后续用户验收步骤（现在不执行）

1. 确认 Linggan 本地数据库与 `http://localhost:3000/health` 显示准备就绪。
2. 在 Chrome 扩展页加载/更新本卡已审查并合并的 `v0.3.3` ZIP。
3. 用户自行登录小红书，打开 `ADHD` 搜索结果并亲眼确认“综合”排序；不输入账号信息给 Linggan 或 Agent。
4. 点击“发现当前前 20 条”。页面应先显示“已本机排队”，再显示“正在提交”及结果；发生 4xx 冲突时应要求刷新后重新发起，不能无限重试。
5. 打开 `http://localhost:3000/corpus/evidence`。只检查服务端 `acknowledged` 的卡片是否出现、Coverage 是否如实、封面是否为 `MEDIA NOT ACQUIRED`。若来源发布时间未知，30D 页面必须诚实显示其被发布时间窗口排除，而不是补造时间。
6. 记录真实页面变化与发现结果，不把一次手动采集误称为完整平台、详情、评论、媒体、趋势或研究证明。

## 明确未做

- 自动调度、工位、授权、账号池、旧工作台 API/同步/fallback；
- 详情、评论、作者、媒体下载/本地化、缩略图、OCR、ASR；
- 任意平台总量、缺失对象、趋势、Topic、Insight、Claim；
- 搜索页 HTML 兼容性的真实世界证明。
