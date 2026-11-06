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

## C42 · 空态

- 实现：[apps/web/src/shared/EmptyState.tsx](../../../apps/web/src/shared/EmptyState.tsx)
- 样式：[apps/web/src/shared/EmptyState.module.css](../../../apps/web/src/shared/EmptyState.module.css)
- 变体/触发：first_use / no_match / no_material / unavailable_feature
- 状态：empty_ready
- 负责：空态的层级：标题说缺什么，正文说原因与边界，动作区只放已经可用的下一步；四种空的原因用 data-empty-kind 标注，版式一致
- 页面负责：读取失败、加载中与在途刷新；业务文案与下一步是否真的可执行；表格行内与嵌入卡片内的紧凑空态
- 测试：apps/web/src/shared/EmptyState.test.tsx
- 展示：apps/web/design-system/dsReview/ui.jsx（引用同一真实组件）

## C41 · 读取与刷新状态

- 实现：[apps/web/src/shared/LoadingState.tsx](../../../apps/web/src/shared/LoadingState.tsx)
- 样式：[apps/web/src/shared/LoadingState.module.css](../../../apps/web/src/shared/LoadingState.module.css)
- 变体/触发：initial / refresh
- 状态：initial / refreshing / stale_error / ready
- 负责：首次读取只报读取中：不渲染旧内容，也不把未知填成零；刷新与读取失败时不遮住仍在页面上的上次结果，并说明保留的是旧值；读取失败带原因与重新读取入口，不和空数据合并
- 页面负责：读取时机、查询键与重试策略；业务文案与所读对象是什么；动作回执与提交结果
- 测试：apps/web/src/shared/LoadingState.test.tsx
- 展示：apps/web/design-system/dsReview/ui.jsx（引用同一真实组件）

## C43 · 局部提示与限制

- 实现：[apps/web/src/shared/InlineNotice.tsx](../../../apps/web/src/shared/InlineNotice.tsx)
- 样式：[apps/web/src/shared/InlineNotice.module.css](../../../apps/web/src/shared/InlineNotice.module.css)
- 变体/触发：info / warning / error
- 状态：visible
- 负责：原因、影响与下一步写在同一处；关键限制必须落在文案里，颜色只做加强；error 用 alert 播报，说明与限制用 status 播报；下一步只在已经可用时出现；三种语气的版式一致：左侧竖线加底色，文字区与动作区分开
- 页面负责：整组读取失败与保留旧值（归 C41）；动作受理、应用与未确认（归 C44）；可以长期收起的设计教学与系统策略；业务文案是否准确、下一步是否真的可执行
- 测试：apps/web/src/shared/InlineNotice.test.tsx
- 展示：apps/web/design-system/dsReview/ui.jsx（引用同一真实组件）
