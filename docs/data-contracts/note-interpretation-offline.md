# 每篇解读离线合同

> 状态: 草案
> 最后核对: 2026-10-20
> 适用范围: AI-GEN1 C0 纯合成/mock；不供生产serving使用
> 事实来源: Mog已确认每篇解读语义、note-material-input.v1、note_interpretation_offline源码
> 冲突时以谁为准: 已确认产品语义与源码/测试；长期接入由C1合同承载

`note-interpretation-offline.v1`输入复用A的Material，另带清单版本、skill版本和当前方向成员集合。清单成员列表只是校验作用域，不代表20–40方向完整结构、已发布清单或业务批准；两层CatalogueDraft已验证20–40方向、非空名字/定义及每方向2–3个不重复且属于声明样本的例子；结构通过仍非正式清单。显式合成样本已可按同一asOf冻结成员/材料身份和输入hash；旧结构对照按保留/合并/删除/新增四类完整核对两版方向成员，不允许漏项或重复映射。material ready及原版本合同必须匹配；未知/不就绪不改成已读完。

输出固定`origin=offline_mock`，独立绑定domain/work、snapshotIdentity、清单版本与skill版本，不能重用到其他版本。字段均闭集解析；mock origin不能换成provider，也不存在将其发布到正式结果的入口。material身份是输入声明，纯校验不重新查DB核来源资格、不验证已保存快照或真实ADHD归属，不赋予真实读取/发送资格。

| 字段 | 规则 |
|---|---|
| relevance | related / not_related / unknown分别保留；后两者无方向 |
| primaryDirection / alsoRelated | 各至多一个当前清单成员，两者不同；无主方向与不相关由relevance区分；也相关不进入统计（聚合只算主方向） |
| confidence | certain / possible；不授予可用或上线资格 |
| perspective | 家长亲历、成人自述、专业科普、机构推广、其他、未知分别编码 |
| keywords | 按相关度有序、最多5项；citation必须唯一指向title/cover_headline/body/tags的可用片段；绝对Unicode字符位置与字面必须一致；ASR/图片正文等不作关键词证据 |
| reason | 可空以便清除，最多200个Unicode字符，不截断冒充完整理由 |

`mock_unknown`不推断内容，只返回固定unknown/possible/unknown视角、无方向/关键词和明确离线说明。它证明纯流程可复现，不证明模型输入包已经完成、AI质量、来源内容真实或业务签收。本地输入预览已按title/cover_headline/body/tags/ocr五列选择，保留各列缺失/终态及片段血缘，拒绝列重复、缺列、可用状态与正文矛盾；transcript/作者/评论不进入该包。该包仍是离线预览，不是供应商payload或外发授权，也不重新证明真实材料资格。样本选择采用显式合成成员，不调用真实库或随机抽样。freeze_sample拒绝空集合、重复成员、不就绪/非法输入、domain/asOf/清单或skill版本及成员集混杂；成员按UUID排序，固化fixtureRevision、材料身份/血缘及五列InputPreview JSON的SHA-256。manifestSha256按本v1结构字段顺序序列化、摘要字段留空计算；它是可复现回执，不是签名、数据库快照保存或真实来源资格证明。CatalogueDraft可进一步核对其版本/成员/样例是否与该集合一致。旧结构对照、聚合/裁定/失效及界面在C0范围；C1才接真实外发和调用结算。


## 会话裁定、版本与汇总

ManualJudgment只包含离线origin、原输入Identity、相关性、主方向/也相关和可清空理由；必须按原版本Input验证。投影到新清单/skill版本时保留原记录，方向仍存在才应用到当前机器示例；任一原裁定方向已不在新成员集时显示当前机器示例并needsReview=true，不自动映射到合并方向。domain/work/材料snapshot不匹配则拒绝，不凭旧裁定绕过来源变化。机器示例始终保留，人工分类不改写它。

summarize拒绝重复work及domain/清单/skill版本混杂，先核输出合同再统计。相关且有主方向才累计该方向；也相关不重复累计，未知、不相关、暂无分别计数。点赞未知与0分开，只有已知且严格大于1000/5000进入对应档；发布周是调用者声明的合成字段，未知单列，不从观察时间猜测。关键词每篇去重后按篇数取前5，计数相同按字面排序，不合并同义词。前端读取服务端汇总，不另造统计权威；只有固定合成范围，不具备真实库或市场外推资格。

## 固定合成预演HTTP

`GET /api/local/ai-offline-preview`返回旧版固定示例。`POST`同一路径接收闭集`{scenario: baseline|candidate, judgments: [...]}`，最多6份不重复、精确属于fixture的裁定，16KiB请求上限；未知字段/场景/身份/版本/方向/超长理由拒绝，不接任意材料、skill、文件、供应商或运行参数。两条路径复用现有本机Host/Origin/Sec-Fetch-Site守卫，响应no-store。

返回`note-interpretation-offline-preview.v1`、offline_mock、两版清单/结构对照、当前固定样本摘要、五列输入、原/当前机器示例及会话投影、汇总。fixture为后端手写的6篇合成材料，两版各20方向；其Material身份是明确的synthetic声明，不冒称数据库已保存快照或A的真实指纹算法证明。输入JSON SHA-256绑定本次文字、状态与声明身份。前端的小型组件测试props另作单元边界，不能替代这份后端固定样本。

路由无LocalWebState/Database参数、无SQL/provider/worker/生产任务handler；POST只返回本次纯计算结果，服务端无会话存储。页面仅在内存保留所选裁定，刷新回固定旧版，无localStorage/正式结果写入。C0不提供发布清单、启用调度、真实调用或原文导出口。React和Axum只在自有loopback主路径验证，测试app明确使用NotConfigured数据库状态。
