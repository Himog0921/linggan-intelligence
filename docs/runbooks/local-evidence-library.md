# 本地 Evidence Library 运行手册

> 状态: 权威当前
> 最后核对: 2026-08-30
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

服务会明确绑定 `127.0.0.1:3000`。因此它只供这台 Mac 使用，不会监听局域网或互联网。它不连接旧内容工作台或旧数据库。平台访问只由已签到且已领取服务端 TaskSpec 的 Browser Producer 执行；API、scheduler 与媒体处理 worker 本身不登录平台。

从 `MATERIAL-DEEPENING-001` 起，`serve` 同时启动三类本机进程：loopback API、观察调度 worker 和媒体处理 worker。媒体处理只读取已经物化到 `LINGGAN_LOCAL_MEDIA_ROOT` 的本地字节，并按可用命令启用 PaddleOCR（图片与视频帧）、FFmpeg 缩略图/音频/抽帧和 local Whisper ASR；它不会把原始媒体提交给外部模型 API。若处理器不可用，页面保留 `NOT_ENABLED/QUEUED/UNKNOWN`，不能写成已处理。

未使用本运行手册启动时，Evidence Library 会保持 `SOURCE_INCOMPLETE / NOT_CONNECTED` 的诚实空态。不要把这个状态理解为“世界没有材料”。

日常本地运行必须使用 Linggan 自己的受控入口。它从本机 `.env` 读取 Linggan 的本地数据库配置，确认 migration 已登记、通过 TCP 口令验证后再启动；不会读取旧内容工作台或其数据库：

```bash
./scripts/local-runtime.sh serve
```

该脚本不会把真实连接地址写进终端、Issue、PR 或仓库文件。它用当前 `.env` 的 `POSTGRES_USER`、`POSTGRES_PASSWORD`、`POSTGRES_PORT` 与目标数据库名派生唯一运行目标；不会在 migration 之后改用未经核验的 `DATABASE_ADMIN_URL`。如果外部环境预设的 API 数据库地址与该目标冲突，服务会在启动前拒绝，而不是悄悄连接另一套库。

`/health` 不是仅在启动时检查一次：每次读取都会重新核对数据库与 LOCAL-001 schema。服务运行后数据库失联时，它会从 `READY` 降为 `CONFIGURED_UNAVAILABLE / LOCAL_001_DATABASE_UNAVAILABLE`；这不代表任何已接纳材料被删除，只表示此刻不能诚实读取或接纳。

## 启动

在 Linggan 仓库目录执行：

```bash
./scripts/local-runtime.sh serve
```

看到下面这行后，浏览器直接访问 `http://localhost:3000`；它会临时重定向到 Evidence Library 页面：

```text
Linggan local host listening on http://localhost:3000
```

停止服务时，在同一个终端按 `Ctrl+C`。这不会删除本机已接纳数据，也不会影响任何平台账号或采集任务。

如果本机数据卷来自更早的 Docker 启动，而 `migrate` 明确提示“当前 `.env` 口令不匹配”，不要删除数据卷或 `.env`。先运行一次：

```bash
./scripts/local-runtime.sh repair-password
```

这个显式动作只把本机 `linggan_dev_admin` 角色口令对齐当前 `.env`，随后立即通过本机 TCP 验证；不会重建数据库、清空表或删除数据。完成后再次运行 `serve`。

## 验证本地入口

另开一个终端，运行：

```bash
curl --fail --silent http://localhost:3000/health
curl --head --silent http://localhost:3000/
curl --fail --silent http://localhost:3000/corpus/evidence > /dev/null
```

健康接口会返回机器可读的状态：

- `listener: loopback-only`：表示服务只绑定本机回环地址；
- `database.state: READY` 且 `database.schema: PLUGIN_RUNTIME_002_SCHEMA_READY`：表示此刻 API 已连接由本地运行入口验证的 Linggan 数据库，且自动观察、固定作品深化、媒体取得与本机处理所需 migration 已登记；
- `database.state: NOT_CONFIGURED`：表示没有给服务本地数据库配置；
- `database.state: CONFIGURED_UNAVAILABLE`：表示数据库连接失败或 schema 尚未完成，不能进行 ingress 或读取；具体原因分别在 `database.schema` 中返回 `LOCAL_001_DATABASE_UNAVAILABLE` 或 `LOCAL_001_SCHEMA_UNAVAILABLE`；
- `dataState: MATERIAL_PROJECTION / evidenceReadModel: MULTI_MATERIAL`：只表示多材料只读投影可用，不表示某个作品的每条 lane 已经取得。

根入口的响应应为 `307 Temporary Redirect`，并包含 `location: /corpus/evidence`。`/health` 保持机器可读状态接口，不重定向。

打开 `/corpus/evidence` 后，当前应该看到 Evidence Explorer + Provenance Inspector 工作空间。

- 没有数据库连接时，它说明 `SOURCE_INCOMPLETE / NOT_CONNECTED`；这不能推断世界没有内容或数据库为零。
- 有连接且存在合格 Package 时，页面以作品材料集合展示 discovery、detail、comments、replies、author、media slots/bytes、OCR/ASR。搜索覆盖标题、创作者、详情正文、评论与本机派生文本；它绝不重新搜索小红书。
- URL 未携带 `window` 时，页面采用 `latest_accepted_discovery` 默认读取视角：已接纳 discovery 卡片即使没有来源发布时间也会显示为 `PUBLISHED_AT UNKNOWN`。这不是“最近发布”；页面不会用首次发现、观察、接收或重放时间替代来源发布时间。
- 只有 URL 显式使用 `window=last_7_days` 或 `window=last_30_days` 时，`WINDOW` 才按 ContentItem 身份合并后的来源可直接验证 `published_at` 严格过滤，并以读取时 Linggan PostgreSQL 的 `scope_001_now()` 为唯一时间参照：合并发布时间/未知状态先于文本检索和排序求值；7/30 天窗口只含 `[now - window, now]` 的已知发布时间，未来发布时间不称为最近也不返回。未来记录仍保留为已接纳发现材料；合并后发布时间未知的当前 `EvidenceQuery` 候选集对象不会被填成 0 或“当前”，会在页面明确计为排除对象，即使窗口还有其他可见卡片；有任一已知来源发布时间但不在窗口内的对象仅因超窗不返回，不能误计为未知。文本不匹配的本地对象不能被计入。
- 每张卡片只显示本次 discovery 可见的事实和 package 级 Coverage。`visible / quota` 不是平台总量、完整率或趋势。
- 媒体只在已有合格本地 Materialization 时使用受控本地 URL；短期 CDN 地址只保留为来源观察，绝不作为长期展示回退。Live Photo 的 still/motion 组件分别显示状态；一个组件取得不等于整个卡槽完整。

## 运行环境证明（不写入日常数据）

```bash
./scripts/test-local-runtime.sh
```

这个命令只创建精确命名的临时 proof database：先证明冲突的 API 数据库目标被拒绝，再应用 migration，写入一份合成 discovery Package，启动服务、读取一次、停止并重启服务后再次读取。随后它只禁止该临时库的新连接并终止该临时库的 API 会话，确认 `/health` 不再报告 `READY` 且读取返回 503。结束时它会删除并确认删除这一个临时数据库。它不会停止共享开发库、不访问平台、插件、媒体或日常本地材料。

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

- 不导入或显示任何历史内容工作台数据；
- API 与媒体 worker 不访问小红书；只有领取了受控任务的插件可以打开明确页面和取得获准媒体候选；
- 不因详情、评论、OCR/ASR 可见而自动形成 Topic、趋势、Claim 或自主观察决定；
- 不部署到线上，也不监听 `0.0.0.0`。

Issue #34 实现的是受控 discovery 接纳与只读投影。真实 Linggan Plugin 的 Local mode、真实 `ADHD` 前 20 条 Canary、媒体 acquisition 和 OCR/ASR 仍是独立范围。不要把页面 HTTP 成功或合成 Package 回执当成其中任一链路已经完成。
