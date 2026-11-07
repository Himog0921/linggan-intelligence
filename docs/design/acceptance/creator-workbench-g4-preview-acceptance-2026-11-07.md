# ACC-CREATOR-WORKBENCH-G4 · 隔离 React 预览验收

> 状态: 草案
> 最后核对: 2026-11-07
> 适用范围: `preview/creator-workbench-g4-integration` 开发态预览路由
> 事实来源: SPEC G3 用户批准；Gold Master SHA-256 `c1e564b38811bee179154ac3b2af1847abceab1fa5f44fb67a81d5b958f6350e`；本分支源码
> 冲突时以谁为准: 实际 CI / 浏览器 / 真实代码优先；源码声明不等于运行通过

## Gate 判定

| 检查项 | 当前证据 | 状态 |
|---|---|---|
| DEV-only 路由 | `App.tsx` 以 `import.meta.env.DEV` 条件注册 | 已实现，待 build 验证生产剔除 |
| 唯一共享 Shell | 页面复用 `OpsFrame` / `AppShell` / `Sidebar` / `ContextBar` | 源码已核对 |
| 共享动作与检索控件 | 使用 C09 ActionButton、C16 FindField、C17 SelectField | 源码已核对 |
| LIDS Token | 页面 CSS 使用 `--lgi-*`，Shell 几何为两项有界变量例外 | 源码已核对；lids:check 待执行 |
| 合成数据标记 | 页面明确显示 DEMO / 合成数据；资产详情和动作回执说明预览限制 | 源码已核对 |
| 正式 API / DB 写入 | 页面未导入业务 API 模块；测试断言不调用 fetch | 源码已核对；测试待 CI |
| 资产库交互 | 六类索引、列表、详情、关闭与焦点恢复 | 测试已编写，待 CI / 浏览器 |
| 材料桌交互 | 沉浸切换、拖动、缩放、键盘移动、F1–F4、双击加便签 | 源码已核对；测试覆盖有限，待浏览器 |
| 编辑状态保留 | 写稿/材料桌使用 hidden 保持挂载，草稿 state 不卸载 | 源码已核对；浏览器待验 |
| 六视口视觉回归 | 1440、1280、1024、820、390、375 | 未运行 |
| React 19 typecheck / Vite build | 当前执行环境 Node 22，不满足 Node >=24.13.0 | 未运行 |
| 完整 40 条旅程 | 目前只覆盖 G4 预览切片的关键交互 | 未完成；属于后续 VERIFY |

## 当前阻断

1. 必须等待 GitHub Actions 对本分支实际 head 的 Web / LIDS 检查结果。
2. 必须在 Node >=24.13.0 的目标工程运行 Vite 预览，不能以旧离线 React runtime 或截图代替。
3. 必须完成六视口视觉对照与交互人工复核，尤其是资产弹窗、移动抽屉、画布拖动/缩放和键盘路径。

## 验收边界

通过本表只表示 G4 隔离预览具备继续 VERIFY 的条件，不表示正式创作模块完成，不表示真实业务逻辑、API、数据模型、权限、撤回和发布链路已验收。G6 合并与部署必须另行按用户授权执行。
