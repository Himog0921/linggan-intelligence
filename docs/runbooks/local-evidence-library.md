# 本地 Evidence Library 运行手册

> 状态: 权威当前
> 最后核对: 2026-08-25
> 适用范围: Linggan `apps/api` 的本地 loopback host、`/health` 与 `/corpus/evidence`
> 事实来源: Issue #25、LOCAL-001、当前 Rust 实现与实际运行验证
> 冲突时以谁为准: 实际运行输出、当前代码、LOCAL-001 与用户最新确认；本手册不授予插件或平台访问

## 这台本地服务做什么

它只提供 Linggan 的第一个本地产品入口：

```text
http://localhost:3000/health
http://localhost:3000/
http://localhost:3000/corpus/evidence
```

服务会明确绑定 `127.0.0.1:3000`。因此它只供这台 Mac 使用，不会监听局域网或互联网。它不连接旧内容工作台、旧数据库、真实平台、媒体服务或 AI 服务。

默认启动时，它不连接数据库，Evidence Library 会保持 `SOURCE_INCOMPLETE / NOT_CONNECTED` 的诚实空态。

只有在显式提供 **Linggan 自己的** PostgreSQL 连接地址时，服务才会连接受控的本地 read projection：

```bash
LINGGAN_LOCAL_DATABASE_URL='postgresql://…' cargo run -p linggan-api
```

该变量不能指向旧内容工作台或其数据库。服务不会自动执行 migration，也不会创建数据库；先按数据库交付流程在独立环境中准备好 schema。不要把真实连接地址写进终端截图、Issue、PR 或仓库文件。

## 启动

在 Linggan 仓库目录执行：

```bash
cargo run -p linggan-api
```

看到下面这行后，浏览器直接访问 `http://localhost:3000`；它会临时重定向到 Evidence Library 页面：

```text
Linggan local host listening on http://localhost:3000
```

停止服务时，在同一个终端按 `Ctrl+C`。这不会写入数据库，也不会影响任何平台账号或采集任务。

## 验证本地入口

另开一个终端，运行：

```bash
curl --fail --silent http://localhost:3000/health
curl --head --silent http://localhost:3000/
curl --fail --silent http://localhost:3000/corpus/evidence > /dev/null
```

健康接口会返回机器可读的状态。其中未提供 `LINGGAN_LOCAL_DATABASE_URL` 时：

- `listener: loopback-only`：表示服务只绑定本机回环地址；
- `dataState: SOURCE_INCOMPLETE`：表示这不是数据接入成功的证明；
- `evidenceReadModel: NOT_CONNECTED`：表示 Evidence Library 尚未读取任何材料。

提供有效的 Linggan PostgreSQL 连接地址后，健康接口只会表示 `LOCAL_DISCOVERY_READ_PROJECTION / DISCOVERY_ONLY`：这表示本地读取能力已连接，**不是**真实平台采集、详情 Evidence、评论、媒体或趋势已经完成。

根入口的响应应为 `307 Temporary Redirect`，并包含 `location: /corpus/evidence`。`/health` 保持机器可读状态接口，不重定向。

打开 `/corpus/evidence` 后，当前应该看到 Evidence Explorer + Provenance Inspector 工作空间。

- 没有数据库连接时，它说明 `SOURCE_INCOMPLETE / NOT_CONNECTED`；这不能推断世界没有内容或数据库为零。
- 有连接且存在合格的受控 discovery Package 时，页面只显示本地已接纳的 visible card。搜索框只检索标题和创作者名；它绝不重新搜索小红书。
- `WINDOW` 只按来源可直接验证的 `published_at` 过滤。未知发布时间不会被填成 0 或“当前”，而是在页面明确统计为排除对象；该数量只统计当前 `EvidenceQuery` 候选集，不能把文本不匹配的本地对象计入。
- 每张卡片只显示本次 discovery 可见的事实和 package 级 Coverage。`visible / quota` 不是平台总量、完整率或趋势。
- 封面位置必须显示 `MEDIA NOT ACQUIRED`；此阶段绝不能请求或展示小红书 CDN 地址。

## 受控本地 ingress（仅测试/后续 Linggan 自有插件）

`POST /api/local/discovery-packages` 只接受 `xhs.discovery.visible-card.v1` 合同。它会先验证 Package，再分别保存 Package、逐条卡片、Coverage 和页面投影。

```text
合格的 Package → accepted/replay 回执
不合格的 Package → 422 discovery_contract_invalid
没有本地数据库 → 503 ingress_not_connected
数据库事务未提交 → 503 ingress_not_committed
```

这是 localhost ingress，不是采集命令。它不打开浏览器、不登录账号、不触发真实平台、不会补详情/评论/作者资料/媒体，也不会形成 Observation、Topic、Research、Insight 或市场结论。

## 当前不做什么

- 不显示详情正文、评论、原文、媒体或任何历史工作台数据；
- 不访问小红书，不打开或下载内容详情；
- 不形成 Observation / Topic / 趋势；
- 不部署到线上，也不监听 `0.0.0.0`。

Issue #34 实现的是受控 discovery 接纳与只读投影。真实 Linggan Plugin 的 Local mode、真实 `ADHD` 前 20 条 Canary、媒体 acquisition 和 OCR/ASR 仍是独立范围。不要把页面 HTTP 成功或合成 Package 回执当成其中任一链路已经完成。
