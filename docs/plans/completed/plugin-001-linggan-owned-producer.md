# PLUGIN-001 Linggan 自有浏览器 Producer 包

> 状态: 已完成计划
> 最后核对: 2026-08-25
> 适用范围: GitHub Issue #33 的独立 Manifest V3 浏览器 Producer 基础源码、构建、可安装包、版本与完整性校验；不定义或限制 Issue #37 的后续适配实现
> 事实来源: Mog 已确认 Linggan 自有插件是唯一运行时 Producer、Issue #33、`LOCAL-001C0-DISCOVERY-BOUNDARY-V1`、`LOCAL-001` 活跃计划与 `AGENTS.md`
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、`PLUGIN-MIGRATION-001`、已合入的 Discovery 合同和真实运行证明；历史内容工作台仅为只读参考

## 目标与用户可见结果

Linggan Intelligence 仓库已拥有一份可独立审查、构建和安装的浏览器 Producer 基础包。该基础卡的 `health-only` 停止点已完成；用户随后确认不要从零另造采集器，而应适配当前 `linggan-boom` 的成熟 XHS 能力。因此真实 Discovery adapter、浏览器权限和固定 ingress submit 不再由本历史基础卡定义，统一由 `PLUGIN-MIGRATION-001` 承接。

## 已确认边界

```text
Linggan-owned MV3 browser producer
        ↕ only loopback localhost:3000
Linggan local host / future ingress

历史内容工作台：只读能力与风险参考，不在运行时链路中
```

- 唯一未来运行时 Producer 是 Linggan-owned browser producer package。
- `AcquisitionSpec` 与 `EvidenceQuery` 是不同合同；本插件只为未来受控 Discovery Package 预留序列化，不实现页面库检索或平台命令领取。
- 首批真实 Canary 的未来固定意图是 `xhs / ADHD / comprehensive / maximum_quota / visible_search_card / 20`；本卡不会打开浏览器页面或访问小红书。
- 外部封面 URL 只能作为未来 Discovery Package 的 `MediaCandidate`，永远不能被本插件或产品页面当作本地可展示媒体副本。

## 范围

1. 在 `plugins/linggan-browser-producer/` 建立自包含 TypeScript-free Manifest V3 源码包，避免尚未批准的第三方构建依赖。
2. 使用最小权限 Manifest：仅 Linggan loopback health endpoint 的 host permission；不注入平台页面、不申请 cookies/storage/activeTab/downloads/scripting 等权限。
3. Popup 明确呈现：Linggan Producer 身份、版本、固定 loopback 地址、健康检查结果、Discovery 的 `NOT_CONNECTED` / `NOT_AUTHORIZED` 状态与不可执行动作。
4. 构建脚本从受版本控制源文件生成暂存的 `dist/` 与 ZIP，并生成可核验 release manifest（版本、文件清单、SHA-256）。
5. 静态和构建检查拒绝历史内容工作台依赖、非 loopback endpoint、远程媒体/CDN 回退，以及任何超出 discovery-only 的默认操作。
6. 添加本地安装与验证 runbook，说明浏览器加载、健康检查和真实平台 Canary 是不同证明层。

## 明确不做

- 不实现 localhost ingress/API、数据库、Evidence 接纳、页面读投影或任何 Rust runtime。
- 不实际加载插件、不访问小红书/账号/浏览器，不处理 Cookie、Token、密码、真实原文、详情、评论或媒体。
- 不下载媒体、不建立本地 Blob、缩略图、OCR、ASR、对象存储、隐私/保留/删除传播机制。
- 不从内容工作台复制、import、调用或兼容任何 runtime source、release、endpoint、数据库或队列。

## 文件边界

| 类别 | 文件 |
|---|---|
| Exclusive | `plugins/linggan-browser-producer/**` |
| Exclusive | 本计划、`docs/runbooks/linggan-browser-producer-local.md` |
| Required records | `docs/README.md`、`docs/progress/2026-08.md`、`docs/governance/generated-artifacts-registry.md` |
| Forbidden | Rust/API/worker/DB/UI runtime、旧系统和 `references/`、真实平台与媒体相关文件 |

## 执行与可证伪验收

1. 建立源文件与 Manifest，验证：Manifest V3、最小权限、无内容工作台运行时引用。
2. 建立可重现构建和 release manifest，验证：从 clean source 生成 ZIP；ZIP 内 manifest、版本、文件 digest 与 release manifest 一致。
3. 建立 popup 和 health-only service worker，验证：只有 `localhost:3000/health` 会被请求；健康失败时 Discovery 仍禁用并显示 `NOT_CONNECTED`。
4. 建立静态安全检查，验证：拒绝非 loopback host、旧工作台字符串、远程 image/CSS URL、详情/评论/media/OCR/ASR 默认动作。
5. 运行仓库治理检查，验证：计划、runbook、生成物、索引和月度记录完整。

## 停止与升级

以下任一情况发生时，停止本卡并建立下一张卡：需要真实 ingress 路由、实际浏览器加载、任何小红书 host permission、平台/账号访问、扩展 Discovery 合同、读取/保存真实材料、媒体字节、跨域连接或未确定的隐私策略。

## 完成边界

本卡完成只能证明 Linggan 拥有独立、可构建且受限的插件包。它不能证明浏览器已加载、loopback 已接通、Discovery Package 能被接纳、真实平台采集、媒体取得、OCR/ASR、页面真实展示、部署或业务验收。
