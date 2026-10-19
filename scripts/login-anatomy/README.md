# 登录装置源

> 状态: 代码事实优先
> 最后核对: 2026-10-13
> 适用范围: LOGIN-ANATOMY-001 生成器与页面输入
> 事实来源: Mog 确认的 Login V2、Anatomy 技能、相邻源码
> 冲突时以谁为准: 用户最新确认与运行源码；本文件不授予部署或认证权限

WEB-REACT-001 H 后，页面唯一实现为 `apps/web/src/pages/login/` 的 React/TypeScript 与 CSS Module；旧HTML/原生表单与生命周期脚本已删除。此目录只维护几何、16秒模型、真实遮挡顺序与静态素材生成。Anatomy kit 来自 `wheresryan22/anatomy`，MIT 许可见 `ANATOMY-LICENSE`。

仓库根目录执行：

```bash
node scripts/login-anatomy/build.mjs --audit
node scripts/login-anatomy/verify-scene.mjs
```

需 Node.js 20.10+ 与已安装 Anatomy 技能，默认查找 `~/.codex/skills/anatomy`，可用 `ANATOMY_SKILL_ROOT` 覆盖。Git 生成物为 `apps/web/src/pages/login/assets/{station.svg,scene-data.json}` 及 `sceneModel.ts`；审计、静态 SVG 和 manifest 写入系统临时目录，可用 `LINGGAN_LOGIN_AUDIT_DIR` 指定本任务私有位置。

React生产构建包含页面逻辑、样式、静态帧与原插画降级资源，可离线使用。正式部署由Axum精确入口提供同一个React应用，`/` 转到 `/login`。开发提交仅做前端导航，无认证请求、Cookie 或密码存储；实际认证仍未实现。直接工作台链接仍可访问。

实际页面/表单/窄屏/减少动态效果验证由 `apps/web/test/e2e/login-react.mjs` 承接；须明确指定本机候选、可选旧基线和private证据目录。旧 browser-checks/test-preload 原生DOM验证已删除。
