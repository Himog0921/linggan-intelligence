# LIDS 标准件实现目录（生成）

> 状态: 权威当前
> 最后核对: 2026-10-20
> 适用范围: 已接通同源展示的标准件；不授予CMP
> 事实来源: component-contracts.json；lids:sync生成
> 冲突时以谁为准: 正式规则源和业务合同

## C09 · 动作按钮

- 实现：[apps/web/src/shared/ActionButton.tsx](../../../apps/web/src/shared/ActionButton.tsx)
- 样式：[apps/web/src/shared/ActionButton.module.css](../../../apps/web/src/shared/ActionButton.module.css)
- 变体/触发：primary / ink / outline / soft / danger / quiet / loading
- 状态：idle / focus / disabled / pending / rejected / unconfirmed
- 负责：原生button与显式type；忙碌禁用与aria-busy；M2材质和标签宽度
- 页面负责：导航资格；权限判断；真实成功回执
- 测试：apps/web/src/shared/ActionButton.test.tsx
- 展示：apps/web/design-system/dsReview/ui.jsx（引用同一真实组件）

## C16 · FIND检索输入

- 实现：[apps/web/src/shared/FindField.tsx](../../../apps/web/src/shared/FindField.tsx)
- 样式：[apps/web/src/shared/FindField.module.css](../../../apps/web/src/shared/FindField.module.css)
- 变体/触发：submit / debounce_250
- 状态：idle / composing / querying / results / empty / error / disabled / pending
- 负责：FIND识别与清空/快捷键；IME保护、防抖及手动提交；外部查询/范围改变取消旧定时器；flush与focus
- 页面负责：业务查询结果；URL编码和返回历史；作者/作品资格；创建搜索能力
- 测试：apps/web/src/shared/FindField.test.tsx
- 展示：apps/web/design-system/dsReview/ui.jsx（引用同一真实组件）
