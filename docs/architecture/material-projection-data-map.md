# 插件产出到 Evidence 材料投影映射

> 状态: 权威当前
> 最后核对: 2026-08-31
> 适用范围: 当前 Browser Producer 实际产出、服务接纳、Material Projection 与本机 Evidence API
> 插件运行合同版本: `v0.8.19` 候选；scheduled TaskSpec 身份贯穿、详情/评论/回复/作者/媒体槽位执行、部分材料回执、媒体工作与统一资源读取共用同一受控链。
> 事实来源: `plugins/linggan-intelligence-browser/src/linggan/producerRuntime.js`、`contentRuntimeAdapter.js`、Rust 接纳/投影代码、migration 0015–0027 与隔离 PostgreSQL fixture
> 冲突时以谁为准: 当前插件合同与源码、Rust 运行时校验、数据库约束和真实测试结果

这张表只描述当前已接线的数据路径。插件负责冻结来源 Package 和 Coverage；服务端负责准入、稳定身份、材料投影、受控读取与处置。未知字段保持未知，`batch_checkpoint` 只进入来源血缘，不制造作品材料或整体完成。

| 插件实际产出 | 服务接纳条件与去向 | Material Projection | Evidence 列表 / 详情表达 | 代表性 fixture |
|---|---|---|---|---|
| `discovery_search` / `discovery_card` | Task、Package、Record 的平台/能力/目标/sourceObject 一致；不合格 record 单条隔离 | 稳定作品身份 + `discovery` lane + append-only discovery finding | 默认列表可检索标题/作者与发现时间；50 条 keyset；不混入 legacy cards | `material_projection_postgres.rs`、`material_cursor_tests.rs` |
| `profile_discovery` / `profile_discovery_card` | 目标作者与 record 来源身份一致；只接纳当前可证明的作品卡 | 与搜索发现共用作品身份；`Task/Lease/WorkOrder/Target` 或稳定 author target 精确匹配形成 collection context | 显示“来自监控目标”但不把目标名写成作品作者；身份一致必须等待详情 author ID | `collection_dispatch_sequence_postgres.rs` |
| `content_detail` / `content_detail` | 作品 sourceObject 必须稳定且与 Task target 一致；时间断言必须携带实际字段、kind、precision 与 parser version | 逐字段状态化标题、正文、作者；平台 epoch 可形成精确 `published_at`，可见时间文本单独保留 | 列表只给摘要字段；`/{publicRef}` 返回详情与字段来源；`SOURCE_TEXT_ONLY` 不进入严格时间窗 | `material_projection_postgres.rs`、`material_projection_tests.rs` |
| `comments` / `comment` | 评论稳定身份；正文和作者字段内部保留；同包重复与冲突单条隔离 | `comments` lane、稳定评论材料、Coverage | 普通列表只显示计数/状态/受限访问；`/{publicRef}/comments` 是本机授权研究通道，最多 20 条一页，返回原文、匿名作者上下文和 cursor，不返回平台用户标识 | `material_social_postgres.rs`、`material_projection_tests.rs` |
| `replies` / `reply` | reply root/parent 来源字段、自引用和冲突关系通过运行时与 DB 双层约束 | 与评论共表但保留 `is_reply`、root/parent 关系和独立 replies Coverage | 普通列表只给 replies 状态/计数；研究通道以 `ROOT/REPLY` 关系表达，不暴露外部身份 | `material_social_postgres.rs` |
| `author_profile` / `author_profile` | 作者 sourceObject 与 Task author target 一致 | 作者资料按观察版本追加，不覆盖旧版本 | 详情 Inspector 给当前 as-of 作者上下文与来源引用；外部作者 ID 不作为普通响应字段 | `material_social_postgres.rs` |
| `media_slots` / `media_slot` | 作品媒体按 content 身份校验；详情作者头像还必须同时携带稳定 author 身份和当前 content 上下文；slot、observationRef、每用途 ordinal、候选集合和组件合同通过；重复/冲突逐 record 隔离 | Slot、来源 generation、多 candidate、Live Photo partial、下载/副本/派生生命周期；新接纳媒体建立 `author.avatar/content.cover/content.image/content.video` 权威关系；头像不排 OCR/ASR | Work Resource 统一返回 `media`；avatar 与全部内容槽位只返回 `INLINE_SAFE` 本地句柄；封面只按显式封面 → 首张正文图 → 视频 poster → 无封面选择，不读取候选远程 URI | `linggan-producer-runtime.test.mjs`、`material_projection_postgres.rs`、`material_media_delivery_tests.rs`、`material_replica_fallback_tests.rs` |
| `acquireMediaSlots` 后台上传（不是新的 Capture Package） | 分块上传使用声明 hash/size；finalize 以流式 hash + 精确 size 验证后原子提升到内容寻址路径，再写 Blob/Materialization | `media_bytes` lane 通过下载、Blob、Materialization 和处理事件表达 | 只返回 Materialization/Derivative 绑定的 loopback handle；GET 有可配置大小上限、流式响应、`no-store`/`nosniff`；未知/SVG/HTML 等声明类型可保存但只能 attachment，标记 `UNSUPPORTED_MEDIA_TYPE` | `material_media_delivery_tests.rs`、`material_asset_route_fixture.rs` |
| `batch_checkpoint` | 只接纳执行 checkpoint 与 Coverage 来源，不生成 typed content | 不生成作品材料或 overall completeness | 仅在 provenance/checkpoint 血缘使用 | Producer ingress fixture |

## 当前读取边界

- `/api/local/work-resources` 是跨 Intelligence 页面的唯一作品资源列表；列表项以同一前缀的 `detailUrl` 进入详情，不携带 Inspector。
- `/api/local/work-resources/{publicRef}` 与 `/comments` 是共享详情/授权评论通道；页面不得另读表或 Package JSON 拼标题、作者、时间、封面。
- 统一媒体关系固定为 `author.avatar`、`content.cover`、`content.image`、`content.video`、`content.ocr`、`content.transcript`、`comment.image`；新接纳事实写权威关系，旧材料只在共享读模型内部按既有 purpose 兼容，不要求破坏性回填。
- `media` 同时表达资源关系、可用状态、受控本地句柄和原始尺寸/时长事实；Evidence 列表与 Inspector 必须读取它。`preview` 仅为旧调用兼容字段，不得成为新业务页面的媒体来源。
- `/api/local/evidence-library/legacy` 是显式兼容入口，最多返回 50 张旧 discovery cards；默认入口不混读。
- `snapshot_at/asOf` 只冻结分页中可见的已接纳材料；媒体处置始终按当前 `recorded_at + effective_at` 资格求值，所以旧 cursor 不能绕过已生效撤回。
- 本卡没有实现 OCR/ASR provider、清理 worker、处置管理 UI、企业权限或 Issue #89 的 N+1 批量化。
