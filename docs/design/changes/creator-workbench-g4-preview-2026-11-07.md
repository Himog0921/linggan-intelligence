# CREATOR-WORKBENCH-G4 · React 隔离预览实现

> 状态: 草案
> 最后核对: 2026-11-07
> 适用范围: Gold Master 创作工作台的开发态隔离预览；不代表正式创作页面重构、数据接入或生产发布
> 事实来源: 用户批准的 Prototype-to-Product-Migration Skill；Gold Master SHA-256 `c1e564b38811bee179154ac3b2af1847abceab1fa5f44fb67a81d5b958f6350e`；SPEC G3 已批准；Intelligence main 基线 `a18a439ee69763e6dcfa8b7b3047aaaa489a61aa`
> 冲突时以谁为准: 用户最新确认、真实代码/测试、Gold Master 与已批准 SPEC；合成预览数据不构成业务事实

## 本轮结果

在 `apps/web` 中增加 Vite development-only 路由 `/__preview/creator-workbench`，由 React + TypeScript 实现，使用现有 `OpsFrame` / `AppShell` 作为唯一外壳。该路由只在 `import.meta.env.DEV` 时注册，不进入生产路由表，也不新增 Axum 路由。

页面覆盖写稿三栏、六类资产库 Modal、沉浸材料桌、对象选择/移动/缩放/检查器，以及移动端队列和检查器抽屉。演示数据均为合成数据。页面没有正式 API、数据库、模型、采集、剪贴板发布或外部副作用调用；未接入正式稿件和资产服务。

## 共享能力复用

| 能力 | 复用实现 | 责任边界 |
|---|---|---|
| 全局 Shell / 运营 Frame | `OpsFrame` → `AppShell` → `Sidebar` / `ContextBar` | 唯一页头、一级导航、运营侧栏；材料桌沉浸模式复用既有 `ops-immersive` |
| 动作按钮 | `ActionButton`（C09） | 仅按钮视觉与原生按钮行为；不代表命令已执行 |
| FIND 检索 | `FindField`（C16） | 250ms debounce 与 IME 保护；结果仍由预览页面合成 |
| 范围选择 | `SelectField`（C17） | 只提供选择控件；不代表真实材料范围 |
| 空态 / 提示 | `EmptyState` / `InlineNotice`（C42/C43） | 只表达预览空态和合成限制 |

正文编辑器、材料桌空间画布、资产检查器采用页面局部组件。现有 `ManuscriptEditor`、`MaterialBrowser`、`CanvasBoard`、`CheckPanel` 分别绑定当前稿件数据模型、正式 API 或不同的五列材料模式，不能直接作为 Gold Master 的无损视觉替身。本轮没有复制或改写这些正式组件，也没有晋升新 CMP。

## 受控 Shell 例外

Gold Master 桌面参考视口需要 68px 全局导航轨 + 53.69px Context Bar。预览根作用域仅配置既有 `--v7-global-row` 与 `--v7-context-row` 两个 Shell 布局变量；Shell DOM、`shell.css` 选择器、全局 Token 源均未修改。该例外登记在 `docs/design/changes/lids-component-change.json`，范围仅为此开发态预览路由，不适用于正式 `/ops/creation` 或其他页面。

页面其余颜色、字体、边框、阴影、间距和焦点均消费现有 `--lgi-*` Token。没有新建第二套全局主题或 Shell。

## 运行

在满足仓库 Node 版本要求（`apps/web/package.json`：Node `>=24.13.0`）的开发环境中：

```bash
cd apps/web
npm install
npm run dev
```

访问 `http://127.0.0.1:5173/__preview/creator-workbench`（以 Vite 实际输出端口为准）。此路由仅用于本地开发预览。

## 业务与数据边界

- 六类资产数据全部为 synthetic fixture；特别是“风格”只用于展示批准 Demo 的视觉形态，不表示后端已存在可持久化风格资产合同。
- 材料卡、选题框、便签的坐标、缩放和平移只保存在当前 React state；刷新后不保证保留，也没有数据库持久化。
- “保存、复制、存为样板、用这组材料开稿”等未接真实业务合同的动作只显示“隔离预览、未调用正式服务”的提示，不伪造成功回执。
- 预览不调用 `/ops/api/creation/*`、模型、采集工位或发布接口。
- 不包含 Gold Master 的原始个人材料、图片或完整 DOM 快照；预览文案和封面均为合成样例。

## 尚未完成的门

- 目标仓库 React 19 / Vite 8 下的 `typecheck`、`lint`、`test`、`build` 与 `lids:verify` 必须以本分支实际 CI 为准；本执行环境 Node 为 22，低于项目要求 24.13.0，不能伪称本机通过。
- 需要在真实 Vite 开发服务中运行本路由，检查 1440/1280/1024/820/390/375 六种视口，完成截图比对和关键交互手工复核。
- G4 通过后仍需独立 VERIFY：真实正式组件/API/领域行为不由该预览替代。
- 未获 G6/INTEGRATE 授权前，不合并 main、不部署、不接生产数据。

## 变更文件

- `apps/web/src/app/App.tsx`：只在 DEV 注册预览路由。
- `apps/web/src/pages/workbench/CreatorWorkbenchPreviewPage.tsx`：页面与合成交互。
- `apps/web/src/pages/workbench/CreatorWorkbenchPreviewPage.module.css`：页面局部布局，使用 LIDS Token。
- `apps/web/src/pages/workbench/CreatorWorkbenchPreviewPage.test.tsx`：关键预览交互单测。
- `docs/design/changes/lids-component-change.json`：受限 Shell 页面例外。
- `apps/web/design-system/generated/source-lock.json` / `source-data.json`：同步 LIDS 输入指纹。
