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

## ICON-B-001 · B 精简版图标资产

- 实现：[apps/web/src/shared/IntelligenceIcon.tsx](../../../apps/web/src/shared/IntelligenceIcon.tsx)
- 样式：[apps/web/src/shared/IntelligenceIcon.css](../../../apps/web/src/shared/IntelligenceIcon.css)
- 变体/触发：evidence / comments / creators / sidebar-close / sidebar-open / clock / info / check / chevron / refresh / download / copy / expand / close / normal / wide / focus
- 状态：idle / pointer_hover / keyboard_focus / expanded / disabled / reduced_motion
- 负责：本地SVG几何、统一线宽与两层结构；有限反馈与减少动态效果；装饰图形不占读屏名称
- 页面负责：按钮资格与点击命令；业务执行进度、成功回执和权限；将所有LIDS组件宣布CMP认证
- 测试：apps/web/src/shared/IntelligenceIcon.test.tsx
- 展示：apps/web/design-system/dsReview/ui.jsx（引用同一真实组件）

## C26 · 展示方式切换

- 实现：[apps/web/src/shared/ViewSwitch.tsx](../../../apps/web/src/shared/ViewSwitch.tsx)
- 样式：[apps/web/src/shared/ViewSwitch.module.css](../../../apps/web/src/shared/ViewSwitch.module.css)
- 变体/触发：text_switch
- 状态：selected / unavailable
- 负责：同集合视图切换的文字页签外观与选中/不可用态；aria-pressed 与分组命名；可选读数的位置
- 页面负责：业务命令与执行；范围、查询与数据资格；筛选命中数
- 测试：apps/web/src/shared/ViewSwitch.test.tsx
- 展示：apps/web/design-system/dsReview/ui.jsx（引用同一真实组件）

## C17 · 选项选择

- 实现：[apps/web/src/shared/SelectField.tsx](../../../apps/web/src/shared/SelectField.tsx)
- 样式：[apps/web/src/shared/SelectField.module.css](../../../apps/web/src/shared/SelectField.module.css)
- 变体/触发：select
- 状态：ready / disabled / unknown_value
- 负责：单值选择器的触发器外观、弹出层定位与翻转；compact 只收紧尺寸，不改语义；键盘开合、方向键、首字母检索与 Escape；选中标记与焦点回到触发器；未知原选值原样显示，不替换成第一项
- 页面负责：查询时机与读取资格；加载中、读取失败与无可选项的解释；选项内容与业务排序；提交、写入与回执
- 测试：apps/web/src/shared/SelectField.test.tsx
- 展示：apps/web/design-system/dsReview/ui.jsx（引用同一真实组件）
