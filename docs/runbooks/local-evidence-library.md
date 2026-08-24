# 本地 Evidence Library 运行手册

> 状态: 权威当前
> 最后核对: 2026-08-24
> 适用范围: Linggan `apps/api` 的本地 loopback host、`/health` 与 `/corpus/evidence`
> 事实来源: Issue #25、LOCAL-001、当前 Rust 实现与实际运行验证
> 冲突时以谁为准: 实际运行输出、当前代码、LOCAL-001 与用户最新确认；本手册不授予插件或平台访问

## 这台本地服务做什么

它只提供 Linggan 的第一个本地产品入口：

```text
http://localhost:3000/health
http://localhost:3000/corpus/evidence
```

服务会明确绑定 `127.0.0.1:3000`。因此它只供这台 Mac 使用，不会监听局域网或互联网。它不连接旧内容工作台、旧数据库、插件、真实平台、媒体服务或 AI 服务。

## 启动

在 Linggan 仓库目录执行：

```bash
cargo run -p linggan-api
```

看到下面这行后，浏览器访问页面：

```text
Linggan local host listening on http://localhost:3000
```

停止服务时，在同一个终端按 `Ctrl+C`。这不会写入数据库，也不会影响任何平台账号或采集任务。

## 验证本地入口

另开一个终端，运行：

```bash
curl --fail --silent http://localhost:3000/health
curl --fail --silent http://localhost:3000/corpus/evidence > /dev/null
```

健康接口会返回机器可读的状态，其中：

- `listener: loopback-only`：表示服务只绑定本机回环地址；
- `dataState: SOURCE_INCOMPLETE`：表示这不是数据接入成功的证明；
- `evidenceReadModel: NOT_CONNECTED`：表示 Evidence Library 尚未读取任何材料。

打开 `/corpus/evidence` 后，当前应该看到 Evidence Explorer + Provenance Inspector 的空态工作空间。它会说明“当前没有可展示的本地已接纳材料”，同时说明这**不能**推断世界没有内容或数据库为零。

## 当前不做什么

- 不显示真实帖子、评论、原文、媒体或任何历史工作台数据；
- 不接收插件上传，不访问小红书，不打开或下载内容详情；
- 不写数据库、不会形成 Evidence / Observation / Topic / 趋势；
- 不部署到线上，也不监听 `0.0.0.0`。

材料只读投影属于 001B；插件的 Local Linggan mode 和一次受控 discovery Canary 属于 001C。不要把本页的 HTTP 成功当成其中任一链路已经完成。
