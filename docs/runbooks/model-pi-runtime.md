# 模型与 Pi 本机运行说明

> 状态: 权威当前
> 最后核对: 2026-09-06
> 适用范围: MODEL-PI-001 交付分支的依赖准备与隔离预览，不自动授权共享部署
> 事实来源: .nvmrc、固定 npm lock、实际 runtime / preview 脚本
> 冲突时以谁为准: Mog 对具体 HEAD/共享迁移/运行切换的授权与真实运行证据

正式凭据由 API 写入 macOS Keychain，service 是 `Linggan.Intelligence.Models.<workspace UUID>`，account 为随机不可变 secret UUID；不枚举 Keychain、不保存 API key 到环境文件或数据库。非 macOS 或 Keychain 失败关闭调用，不降级明文。

运行需要 `.nvmrc` 的 Node 24.13.0。默认路径 `$HOME/.nvm/versions/node/v24.13.0/bin/node`；可明确设置绝对 `LINGGAN_PI_NODE`，实际版本仍须匹配。`./scripts/runtime/prepare-pi-adapter.sh --install` 仅在当前 checkout 内按 lock 执行 `npm ci --ignore-scripts`，验证两包精确 0.85.1，并写 gitignored 安装 hash。`--check` 只读验证，不访问模型或数据库。

常驻服务的 sync 在构建三个 Rust binary 后准备依赖，launch 显式传固定 Node 路径并检查。适配器脚本从构建时 checkout 的 `apps/pi-adapter/src/adapter.mjs` 定位，因此 binary 必须连同构建 checkout 使用，不能单独搬到另一目录后删除来源。runtime-main 当前流程满足这个路径布局；升级仍须 Mog 单独授权，本文不能证明已经切换。

migration 0040 已登记 `local-runtime.sh migrate`；它依赖 #168 的 0039。服务不会自行迁移共享数据库。评论模型循环已在 `linggan-worker` 内独立异步组合，巡检任务不 await 模型；10 秒检查已授权计划。读页面/保存配置不创建授权。暂停连接/计划阻止之后的派发，已发送的远端请求不保证撤销。心跳 90 秒不新鲜时页面说明未获当前运行证明。

`linggan-comment-worker --execute [--once]` 为单独入口，消费相同账本，受同一并发/预算检查；`--queue-only [--once]` 保留旧无模型同步行为。在正常服务已经执行时无需另起一个常驻评论进程。

## 隔离验证

1. `./scripts/runtime/prepare-pi-adapter.sh --install`（仅本 checkout 的 npm 依赖）。
2. `npm test --prefix apps/pi-adapter`：真实 SDK 对本机 SSE fixture，包含三种支持协议、错误/重定向、未知用量及无限流超时。
3. `./scripts/test-model-pi-postgres.sh`：随机独立 PostgreSQL/container/volume，完整 migration 与合成材料；trap 清理。无需项目 `.env` 或共享数据库。
4. `cargo test -p linggan-intelligence --test model_keychain --locked -- --ignored`：只写读更换删除随机 service/account 的合成 secret，不枚举/读取已有项。
5. `./scripts/preview-model-pi.sh`：创建独立 PG、合成评论、真实 API、独立评论 worker 和本地合成供应商，输出设置 URL/provider URL/PID。明确 `LINGGAN_MODEL_SYNTHETIC_PREVIEW=SYNTHETIC-NOT-EVIDENCE`，只接纳公开 marker `SYNTHETIC-NOT-A-CREDENTIAL` 和本机地址。这个 store 不接受真实 key。
6. 在新预览上运行 `node scripts/verify-model-pi-api.mjs <origin> <provider URL>`；脚本先验证合成存储与每条来源标识才写入。它配置一个模型并执行一条合成来源；要做从空设置开始的浏览器验收，应重建预览。

关闭预览所属 API PID 会让脚本退出并清理它拥有的 worker、fixture、container、volume。脚本保留 `/tmp` 下的合成日志，供审核定位。不要用 shared `:3000` 或真实库 URL 代替。

真实外部首次试验：在已获部署/运行授权的配置页由 Mog 输入自己的供应商、准确模型 ID 与凭据，先合成能力测试，再明确选定一条已许可原声及 token 额度试运行。保存本身不授权全库；历史与自动新增分别启用。外部内容处理和费用授权由 Mog 决定，本包没有代填或搜寻其它项目凭据。
