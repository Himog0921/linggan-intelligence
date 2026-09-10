# 模型与 Pi 本机运行说明

> 状态: 权威当前
> 最后核对: 2026-09-10
> 适用范围: COMMENT-RESEARCH-RESET-001 V1 的 Pi adapter 依赖、模型连接与受控调用
> 事实来源: `.nvmrc`、固定 npm lock、V1 worker/API 合同与本机 runtime 部署手册
> 冲突时以谁为准: 用户对真实外发/费用的授权、真实运行回执和当前代码

模型设置只管理连接、模型、V1 默认研究模型、embedding 配置及不可变调用账本。保存配置或进入评论研究不会发送评论；持续自动排程没有入口。

## 运行依赖

Pi adapter 使用 `.nvmrc` 固定的 Node 版本。运行：

```bash
bash scripts/runtime/prepare-pi-adapter.sh --install
bash scripts/runtime/prepare-pi-adapter.sh --check
npm test --prefix apps/pi-adapter
```

安装只在当前 checkout 的 `apps/pi-adapter/node_modules/` 执行 `npm ci --ignore-scripts`，并写入 gitignored 安装 hash；不读取模型凭据、不访问数据库或调用 provider。adapter 由构建该 Rust binary 的 checkout 定位，不能脱离来源目录单独搬运。

正式凭据由 API 写入 macOS Keychain，service 为 `Linggan.Intelligence.Models.<workspace UUID>`，account 是随机不可变 secret UUID。不得将 API key 写入环境文件、数据库、日志、Git 或测试 fixture；Keychain 不可用时关闭调用，不降级为明文。

## V1 调用边界

1. 用户在模型设置保存并测试可调用的生成模型，以及独立的 embedding 配置。
2. 用户在评论研究保存一次 policy（研究模型与单轮总 Token 限额）。这只保存策略；语义提取、向量候选与问题归并的每笔调用都附属该 Run 并写入通用调用账本。
3. 用户点击“开始研究”后，V1 冻结当前普通用户且可读的评论；`linggan-comment-worker --execute [--once]` 依次执行语义提取、embedding、候选归并和结果发布。
4. 生成模型只输出 semantic Atom 或 same/new problem JSON；embedding 只返回向量。统计、membership、变化与发布由 Rust/SQL 校验和写入。
5. 每次调用落入通用 `linggan_model_invocation`：请求 hash、模型/连接版本、预留与实际用量、provider 失败及 V1 stage/run reference 可审计。adapter/provider 的短暂失败最多重试到队列上限；不兼容输入/配置与无效输出终态留痕，不阻塞后续 Item。

真实评论外发、费用和语义质量仅在用户主动开始一轮研究后产生；部署、保存策略、调用测试和本 runbook 不构成此授权。供应商测试应使用最小 synthetic input；真实材料需遵守当前用途、最小样本与输出边界。

## 验证边界

- `bash scripts/test-comment-research-postgres.sh`：随机隔离 PostgreSQL 的 V1 schema、queue、poison isolation、结果与 route proof；不访问共享数据库或 provider。
- `npm test --prefix apps/pi-adapter`：Pi SDK/transport/structured-output fixture proof；不含真实凭据或评论。
- `cargo test -p linggan-intelligence --test model_keychain --locked -- --ignored`：只读取、替换、删除随机 synthetic Keychain secret，不枚举已有秘密。
- `docs/runbooks/local-runtime-deployment.md`：共享开发库 migration、runtime 切换与 :3000 验收的唯一运行步骤。

这些证明不等于真实 provider 可用、真实评论语义正确或用户业务验收。
