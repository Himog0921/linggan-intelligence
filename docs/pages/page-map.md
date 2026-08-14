# 页面地图

页面设计服务于情报工作流，不以旧工作台导航为模板。

## P0 页面

| 页面 | 用户问题 | 核心对象 |
|---|---|---|
| 情报收件箱 | 今天哪些变化值得看？ | Signal、Intelligence Event |
| 情报详情 | 结论依据是什么，是否可信？ | Evidence、反例、置信度、建议 |
| 观察计划 | 为什么观察、观察谁、多久一次？ | Observation Target/Plan |
| 观察对象档案 | 这个对象随时间发生了什么？ | Observation Timeline |
| 证据浏览器 | 当时真实抓到了什么？ | CapturePackage、Evidence、Coverage |
| 执行工位 | 哪些插件在线、正在观察什么？ | Station、Lease、Run、Failure |
| 行动与结果 | 基于情报做了什么，结果怎样？ | Action、Outcome |

## 页面共同规则

- 页面展示的确定事实必须带来源。
- 未知不能显示为 0、没有或正常。
- Projection 缺失时显示明确不可用，不回退旧字段。
- 所有 AI 文本必须能展开所使用 Evidence、反例与缺失。
- 页面不直接读取原始 Evidence 表，由受控查询服务返回版本化 DTO。

## 后置页面

- Cluster 演化地图。
- 用户群体档案。
- 预测与红队推演。
- 内容/产品/运营行动 Agent。
