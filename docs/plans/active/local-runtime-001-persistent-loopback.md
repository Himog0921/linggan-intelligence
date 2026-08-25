# LOCAL-RUNTIME-001：持久本地 Evidence Library 运行环境

> 状态: 活跃计划
> 最后核对: 2026-08-25
> 适用范围: Linggan 本机 PostgreSQL、两份已获准 migration、loopback API 与 Evidence Library 的可重启本地运行链
> 事实来源: Issue #38、`compose.yaml`、`scripts/local-runtime.sh`、`apps/api/src/local_web.rs` 与 `scripts/test-local-runtime.sh`
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、实际运行结果和当前代码；本计划不授权插件或真实平台访问

> 当前实施水位: Issue #38 / Draft PR 待独立审查

## 用户结果

Linggan 在本机以一个独立、持久的 PostgreSQL 16 运行。启动服务前，系统会明确应用并登记当前两份获准 migration；`/health` 只有在数据库可连接且这两份 migration 与所需表均存在时才报告 `READY`。停止再启动本地服务不会删除已经接纳的 Linggan 数据。

## 允许范围

- Docker Compose 已命名的 Linggan 本地数据卷；
- `0001_scope_001_capture_evidence.sql` 与 `0002_local_001_discovery.sql` 的一次性、带 checksum 登记的本地应用；
- `http://localhost:3000` 的 loopback-only API、健康状态、Evidence Library 读取；
- 精确命名的临时 proof database 上的合成 discovery 接纳、服务重启与读取证明，并在结束时删除该临时库。

## 明确不做

- 不修改插件、浏览器、真实小红书、内容工作台、详情、评论、媒体、OCR、ASR 或 AI；
- 不删除、重置、回填或导入日常 Linggan 数据；
- 不部署、不开放局域网/互联网监听，也不写入密码、DSN 或任何真实材料。

## 执行与可证伪验收

1. `./scripts/local-runtime.sh migrate`：启动本地 PostgreSQL，登记并仅应用 checksum 一致的 migration；若持久容器角色口令与 `.env` 不匹配，停止且不写业务数据。
2. `./scripts/local-runtime.sh repair-password`：这是显式恢复动作，仅把本地 `linggan_dev_admin` 角色口令与当前 `.env` 对齐；不重建容器、不删除卷或表，并在随后用 TCP 口令验证。
3. `./scripts/local-runtime.sh serve`：先完成上述安全检查，再以 `LINGGAN_LOCAL_DATABASE_URL` 启动 API；默认仍绑定 `127.0.0.1:3000`。
4. `./scripts/test-local-runtime.sh`：使用严格命名的临时数据库和独立 loopback proof port，验证 migration → 合成 accepted discovery → health `READY` → API readback → 服务重启后相同 readback；清理仅针对该临时库和临时目录，并核验库确实消失。

## 通过与未证明边界

**本卡通过仅表示**本机持久运行环境和 LOCAL-001 discovery read projection 可运行。它不证明浏览器插件已经可采集、真实 `ADHD` 搜索页可用、任何详情/评论/媒体/AI 链路成立，也不证明正式趋势或用户业务验收。
