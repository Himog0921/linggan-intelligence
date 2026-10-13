# 登录装置源

> 状态: 代码事实优先
> 最后核对: 2026-10-13
> 适用范围: LOGIN-ANATOMY-001 生成器与页面输入
> 事实来源: Mog 确认的 Login V2、Anatomy 技能、相邻源码
> 冲突时以谁为准: 用户最新确认与运行源码；本文件不授予部署或认证权限

`page.html` 为已确认页面的维护源，只增加开发模式说明和进入既有 `/corpus` 入口的导航。其他模块分别维护几何、16 秒模型、真实遮挡顺序、材质及生命周期。Anatomy kit 来自 `wheresryan22/anatomy`，MIT 许可见 `ANATOMY-LICENSE`。

仓库根目录执行：

```bash
node scripts/login-anatomy/build.mjs --audit
node scripts/login-anatomy/verify-scene.mjs
```

需 Node.js 20.10+ 与已安装 Anatomy 技能，默认查找 `~/.codex/skills/anatomy`，可用 `ANATOMY_SKILL_ROOT` 覆盖。唯一 Git 生成物为 `apps/api/src/local_web/login.html`；审计、静态 SVG 和 manifest 写入系统临时目录，可用 `LINGGAN_LOGIN_AUDIT_DIR` 指定本任务私有位置。

生成 HTML 包含所有动效、样式、静态帧与原插画降级资源，可离线使用。正式部署由 Rust `/login` 提供，`/` 转到 `/login`。开发提交仅做前端导航，无认证请求、Cookie 或密码存储；实际认证仍未实现。直接工作台链接仍可访问。
