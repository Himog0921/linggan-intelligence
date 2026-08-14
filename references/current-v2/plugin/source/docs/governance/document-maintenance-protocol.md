# 灵感爆爆爆文档维护协议

> 状态：权威当前
> 最后核对：2026-07-07
> 适用范围：插件产品、技术协议、平台调研、发布和 agent 协作说明。

## 1. 维护原则

插件文档要回答四个问题：

1. 当前插件能做什么。
2. 当前代码实际怎么做。
3. 当前版本和发布包是什么。
4. 下一轮 agent 应该先看哪里。

真实代码、真实浏览器调研、`package.json`、`manifest.json` 和 `progress.txt` 优先于旧计划。

## 2. 变更后必须同步什么

| 变更类型 | 必须同步 |
|---|---|
| 任意代码改动完成并验证 | `progress.txt` |
| 用户可见流程、按钮、提示变化 | `docs/product/PRD.md`、`docs/product/APP_FLOW.md`、`docs/product/TEST_CHECKLIST.md` |
| 字段、数据结构、消息协议变化 | `docs/technical/DATA_MODEL.md`、`docs/technical/MESSAGE_PROTOCOL.md`、`docs/technical/AI_READY_DATA_CONTRACT_V1.md` |
| 页面事实、选择器、平台行为变化 | `docs/SELECTORS.md`、`docs/technical/XHS_FIELD_SURVEY.md`、`docs/technical/DOUYIN_FIELD_SURVEY.md` |
| 授权、工位、任务同步变化 | `docs/technical/PLUGIN_AUTHORIZATION_PROTOCOL.md`、`docs/technical/MESSAGE_PROTOCOL.md` |
| 稳定策略、架构转向、长期规则变化 | `docs/decisions/index.md`，必要时补 `docs/plans/active/README.md` |
| 发布版本变化 | `package.json`、`manifest.json`、`releases/`，必要时补 `docs/chrome-web-store/README.md` |
| 文件落位和治理规则变化 | `docs/governance/` 与 `docs/README.md` |

## 3. 入口文件同步规则

`AGENTS.md` 和 `CLAUDE.md` 必须各自能独立说明：

- 插件和内容工作台的边界。
- 本地工作台联调规则。
- 当前事实源优先级。
- 文档同步硬门禁。
- 文件落位规则入口。
- 最低验证方式。

## 4. 当前事实核对方式

不要从旧计划判断当前版本。先核对：

```bash
node -e "console.log(require('./package.json').version)"
node -e "console.log(require('./manifest.json').version)"
ls releases/linggan-boom-v*.zip | tail
```

页面结构、选择器和平台接口必须用真实页面或最新调研文档复核，不凭记忆写实现。

## 5. 收尾检查

完成一轮治理或开发后，至少执行：

```bash
node scripts/check-project-governance.mjs
```

如果检查失败，先判断是本轮新增问题还是既有债务。本轮新增问题必须修；既有债务要在最终回复中说明影响和后续处理顺序。
