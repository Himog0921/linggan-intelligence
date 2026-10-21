# ACC-AI-SETTINGS-LEFT-NAV-001 · AI 设置左栏导航验收

> 状态: 一次性报告
> 最后核对: 2026-10-21
> 适用范围: `/settings/models?view=capabilities|models|runs|advanced` 候选实现；未合并、未部署
> 事实来源: 当前候选源码、聚焦 Vitest、生产构建、隔离 React→Axum→PostgreSQL 浏览器证明及其截图
> 冲突时以谁为准: 用户最新确认、当前代码/运行回执；本记录不替代 Mog 视觉验收或发布回执

## 1. 结论

四个正式 AI 设置视图已从 Context Bar 迁入共享可折叠左栏。主内容从当前视图直接开始；截图中要求删除的 `SYSTEM / AI OPERATIONS`、巨型“AI 设置”、说明文字和“刷新设置”按钮均不再渲染。现有查询地址、读取/失败状态和配置动作没有改动；历史兼容的 `section=` 深链接继续使用原有分区内容与链接结构。

## 2. 已通过的自动与实际浏览器检查

| 层级 | 结果 | 覆盖范围 |
|---|---|---|
| 聚焦组件测试 | PASS，2 文件 / 17 项 | 四个视图的左栏、当前项、无可见 Hero/刷新按钮、详情抽屉、旧深链接及无导航写入 |
| 类型、lint、LIDS 与生产构建 | PASS | `typecheck`、`lint --quiet`、`lids:check`、Vite 生产构建 |
| 隔离浏览器：本次页面合同 | PASS | 自有 PostgreSQL + Axum + 已构建 React；1440×900 与 390×844 的四个 `view=` 入口均使用共享左栏、无可见 Hero/刷新按钮、document/main 无横向溢出、切换不发 POST |
| 截图人工核对 | PASS | `settings-new-capabilities-1440.png` 与 `settings-new-capabilities-390.png`：左栏在桌面固定为模块导航，390px 走共享横向轨道（四个入口为轨道的直接子项）；内容顶部没有重复标题/说明/刷新控件 |

浏览器截图仅存于本次隔离证明目录，未提交 Git：

- `/tmp/ai-settings-left-nav-001-browser-p2/settings-new-capabilities-1440.png`
- `/tmp/ai-settings-left-nav-001-browser-p2/settings-new-capabilities-390.png`
- 同目录还含 models、runs、advanced 的 1440px/390px 截图各一张。

## 3. 已知基线失败与归因

完整 settings 实链在本次页面合同通过、连接保存/能力声明/额度保存三个既有步骤通过后，于“导入做法包”收到 `POST /api/local/ai-settings/skills/import` 的 `400`，并等待“已导入做法”回执超时。这不是本次导航代码触及的接口。

为避免错误归因，另以未修改的 `5997b189` 主线基线在一次性工作树复跑相同设置证明，得到同一导入步骤的 `400`。因此完整 settings 实链在该基线上已失败；本候选不把它报告为通过，也没有改动导入、Pi 适配器或后端来掩盖该问题。

## 4. 未证明与发布边界

- 尚未运行完整 settings 实链至绿色，因为上述主线基线的导入失败仍待独立处理；
- 未对本机 `:3000` 重启、刷新或覆盖；未合并、推送、迁移或修改共享数据库；
- 未调用真实模型、供应商、采集平台或创建研究任务；
- Mog 对正式运行页面的最终视觉与业务验收仍待后续明确授权。
