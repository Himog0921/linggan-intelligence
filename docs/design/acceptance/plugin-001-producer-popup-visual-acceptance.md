# ACC-PLUGIN-001 · Linggan Browser Producer Popup 验收

> 状态: 一次性报告
> 最后核对: 2026-08-25
> 适用范围: Issue #33 popup 的静态 source、release package 与状态边界验收；不替代浏览器或真实采集验证
> 事实来源: `PAGE-PLUGIN-001`、UI Change Manifest、实际 build/static checks 与 Issue #33
> 冲突时以谁为准: 真实插件加载/运行、当前合同和用户确认；本记录不把静态检查说成真实链路

## 验收对象

- Issue / SCOPE：Issue #33 / `PLUGIN-001`。
- 页面/组件/状态：Manifest V3 popup；package、loopback、authorization、Discovery boundary readouts。
- 关联规则：`PAGE-PLUGIN-001`、`LIDS-PAT-001`、`LIDS-PRI-001`、`LOCAL-001C0-DISCOVERY-BOUNDARY-V1`。
- 验收日期与环境：2026-08-25；source and release package only。
- 适用数据/权限前提：无真实数据；manifest 仅允许 `http://localhost:3000/*`。

## 场景矩阵

| 场景 | 用户任务 | 预期状态含义 | 视觉检查重点 | 真实后果/回执 | 结果 |
|---|---|---|---|---|---|
| 初始打开 | 看清 package 与固定目标 | `CHECKING` 不是成功 | L1 单列 readout、disabled button | 不发起平台动作 | PASS（source/release 静态） |
| loopback 不可达 | 知道本机无法接通 | `LOOPBACK_UNREACHABLE` 不等于平台失败 | 文字+危险色 readout | 不允许 Discovery | PASS（source 分支；未在浏览器运行） |
| loopback 可达 | 知道 host 可达但不能采 | `LOCAL_HOST_REACHABLE` 不等于 ingress | `NOT_AUTHORIZED` 与 `NOT_CONNECTED` 常驻 | 不允许 Discovery | PASS（source 分支；未请求真实 host） |
| ingress 未实现 | 不误点采集 | disabled 是真实边界 | 禁用按钮和常驻原因 | 无任务、无提交 | PASS（source/release 静态） |

## 视觉工作条件

- 声明的桌面工作区/视口：未加载浏览器，`NOT VERIFIED`。
- 输入内容长度与数据密度：固定状态文案；无真实材料。
- 已批准设计规则：`PAGE-PLUGIN-001`、L1 Settings/Governance、Token-only CSS。
- LIDS 状态五轴或合成边界依据：连接、授权、ingress 分开表达；不创建合成数据。
- Reduced Motion / 移动或静态 Poster 降级：无动画；实际浏览器走查 `NOT VERIFIED`。
- 截图/录屏：本卡不生成；无生成物。

## 分层结论

| 完成层 | VERIFIED / NOT VERIFIED / N/A | 证据 | 仍有限制 |
|---|---|---|---|
| 设计规格一致 | VERIFIED（静态） | `PAGE-PLUGIN-001`、Token-only CSS audit、禁用动作结构 | 未打开浏览器 |
| 前端/组件实现 | VERIFIED（source/release） | MV3 popup、service worker、packaged LIDS token copy | popup 未实际加载 |
| 自动检查 | VERIFIED | `npm run build`、`npm run check`、release ZIP/hash inspection、governance check | 不证明浏览器实际行为 |
| 真实链路/回执 | NOT VERIFIED | 本卡禁止真实 ingress/采集 | 无 Evidence/媒体/平台副作用 |
| 部署 | N/A | 本地可安装包不等于部署 | 未发布扩展商店 |
| Mog / 业务验收 | NOT VERIFIED | 待用户实际加载后 | 不由静态检查推断 |

## 发现与后续

- 真实 plugin load、host health、ingress、平台 Discovery 和页面数据互通必须由后续独立任务验证。
- 本卡不可据此声称任何小红书搜索、20 条卡片、封面副本或 Evidence Library 真实材料已经存在。
