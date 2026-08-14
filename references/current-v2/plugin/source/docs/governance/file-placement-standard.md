# 灵感爆爆爆文件落位标准

> 状态：权威当前
> 最后核对：2026-07-07
> 适用范围：插件仓库内新增、移动、归档文件。

## 1. 新增文件前先看哪里

新增文件前，agent 必须先确认：

1. `AGENTS.md`：项目边界、事实源和协作铁律。
2. `docs/README.md`：文档地图和状态标注。
3. 本文件：文件放置和命名规则。

如果现有目录没有合适位置，先更新文档地图或询问，不要临时新建顶层目录。

## 2. 根目录只放入口和工程文件

根目录允许长期存在的 Markdown 入口只有：

- `AGENTS.md`
- `CLAUDE.md`

根目录允许的工程文件包括：

- `manifest.json`
- `package.json`
- `package-lock.json`
- `webpack.config.cjs`
- `jsconfig.contracts.json`
- `.gitignore`
- `progress.txt`
- `dist.zip`

评审、计划、调研、交接、发布说明、截图说明都应进入 `docs/` 或 `releases/`，不要散落在根目录。

## 3. 文档放置规则

| 文件类型 | 放置位置 |
|---|---|
| 产品能力、用户流程、验收清单 | `docs/product/` |
| 技术事实、数据模型、协议、平台调研 | `docs/technical/` |
| 当前计划真实状态 | `docs/plans/active/README.md` |
| 已完成计划和历史计划 | `docs/plans/completed/` |
| 决策与长期规则 | `docs/decisions/` |
| 评审和验收报告 | `docs/reviews/` |
| 已被取代材料 | `docs/archive/` |
| Chrome 商店上架资料 | `docs/chrome-web-store/` |
| 文件治理规则 | `docs/governance/` |
| 真实时间线 | `progress.txt` |

`progress.txt` 是当前时间线，不等于“所有细节都放这里”。长篇方案仍应进入 `docs/plans/` 或 `docs/technical/`。

## 4. 源码和测试放置规则

| 文件类型 | 放置位置 |
|---|---|
| Background / Content / Popup / Dashboard | `src/` 对应子目录 |
| 小红书平台能力 | `src/platforms/xhs/` |
| 抖音平台能力 | `src/platforms/douyin/` |
| 页面注入桥接 | `src/injected/` |
| 本地数据层 | `src/db/` |
| 工作台协议和共享常量 | `src/workbench/` / `src/shared/` |
| Node 测试 | `tests/` |
| 调研和验证脚本 | `scripts/` |
| 发布包 | `releases/` |

新增能力优先放入现有平台或工作台模块，不用文件名表达“新版、最终版、临时版”。

## 5. 命名红线

禁止用文件名管理版本：

- `*_v2`
- `*_final`
- `*_old`
- `*_new`
- `*_copy`
- `*_backup`
- `*_draft2`

版本历史交给 git；发布版本交给 `package.json`、`manifest.json` 和 `releases/`。

## 6. 检查命令

```bash
node scripts/check-project-governance.mjs
```

这个检查会核对根目录 Markdown 是否散落、治理文档是否被入口引用、文档地图版本是否跟当前插件版本一致。
