# LIDS 组件同步执行合同

> 状态: 权威当前
> 最后核对: 2026-10-20
> 适用范围: 项目UI标准件变更及设计系统导出；本轮闭合C09/C16
> 事实来源: Mog当前对话确认同步方案并授权实施；LIDS-UI-001
> 冲突时以谁为准: 用户最新明确确认、业务合同、已采用LIDS；本文件约束同步流程

## 先判定变更

1. 页面偏离既有标准：按 implementation_fix 修实现，沿用规则。
2. 已授权共享标准改变：按 shared_standard 同改正式JSON/组件合同、共享实现、适用状态与代表页面。
3. 页面业务例外：按 page_exception 写明页面/区域/条件，保留全局边界。

本轮正式组件参数与行为清单在 component-contracts.json；可读目录由它生成 component-library.generated.md。C09/C16展示直接使用apps/web/src/shared组件；其余C目录仍是合成示例，四类复用和七门槛保持。

## 按需执行

1. 读取命中组件合同、实现、调用方和对应测试；声明受影响页面/区域、行为状态及业务适配。
2. 修改唯一源；C16的查询触发由页面提供，创作者250ms/IME、证据库与任务手动提交保持。
3. 更新本次组件变更声明，写明分类、理由、授权来源和受影响组件；指纹必须对应本次实现。使用lids:record并显式传入--kind、--reason、--authorization；page_exception还要--scope。声明不是自动批准。
4. 在apps/web运行lids:sync生成Token、目录、快照和HTML；运行lids:check、定向测试与lids:build验证可再生。命令定义以package.json为准。正常前端build的prebuild和CI均执行同步检查，存在旧HTML时也核对其来源指纹。
5. 浏览器查看组件状态及代表页面，核对指针/键盘、中文输入、长文本与窄屏。交付当前HTML、来源指纹、影响和未验证范围。

完成条件：受影响组件的声明、合同、实现、测试、生成指纹一致；同源检查通过；代表状态有证据。声明不授予CMP认证，不授权合并/部署/真实命令。

## 渐进读取与版本

项目AGENTS/CLAUDE/codx仅路由本文件；参数只读正式JSON。先按组件编号读取component-library.generated.md，展开对应源和测试。新增标准件闭环时追加component-contracts.json与展示绑定，再纳入检查。

HTML为带来源指纹的可视化导出；修改从规则或组件源进入。当前导出位于dist-design-system；旧final-23是2026-10-20快照，后续不作为实时标准。系统成熟度、组件认证、业务接入、主线和用户验收仍分别记录。

运营实战的P12–P16在catalog-source.json登记implementationSources：真实页面、共享壳、业务Frame/范围适配与CSS一并进入导出输入指纹。页面组合/导航采用既定标准时同步目录与相应PAGE规格；C09/C16仍直接引用真组件，其余代表页是合成组合，输入指纹不表示它们已成为真实业务截图或CMP。P17保留历史定位并标retired，不再列当前入口。
