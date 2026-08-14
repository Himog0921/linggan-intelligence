# 现役 V2 来源说明

## 插件源码快照

- 来源仓库：`/private/tmp/v2-plugin-hotfix-embedded-state`
- commit：`60a896c1def5062dbb8098e05b030c5a0871203b`
- 产品版本：v2.0.95
- 导出方式：`git archive <commit>`
- 文件数：468
- 位置：`plugin/source/`

这是完整 tracked tree 快照，包含插件源码、合同、测试和文档，不包含 `.git`、`node_modules`、`dist`、Cookie 或授权状态。

## 工作台 V2 参考快照

- 来源仓库：`/Users/gongyong/Services/content-workbench/migration/app`
- commit：`2dca6cb9b08cd217d851aef845ea462c19289ef7`
- 导出方式：`git archive <commit> <selected paths>`
- 文件数：113
- 位置：`workbench/source/`

仅导出 Evidence、B2、B3、Media、Projection、Security、Durable Worker、V2 ingress、关联 migration、合同和证明资料。完整旧 `schema.prisma` 只用于依赖审计，不能直接成为新数据库 schema。

## Handoff 与附件

- `handoffs/`：12 份 B3、Release Candidate 和 Hard Cut 历史交付文件。
- `../discussion-attachments/`：38 份与 B1/B2/B3/V2、媒体硬切及运行诊断有关的用户附件。

这些文件可能互相矛盾，因为它们记录不同时间的状态。新项目只接受 `docs/decisions/` 中重新确认的结论。
