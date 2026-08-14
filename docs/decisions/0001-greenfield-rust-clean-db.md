# ADR-0001：Rust Greenfield 与干净数据库

状态：ACCEPTED
确认日期：2026-08-14

## 决定

1. 新项目后端和 worker 使用 Rust。
2. 使用全新 PostgreSQL 数据库，不复制旧表和旧 migration 链。
3. 现役插件和工作台 V2 代码作为参考实现与测试预言机，不作为新 Rust 运行时依赖。
4. 旧数据库作为独立只读 legacy archive；高价值对象通过重新采集进入新系统。
5. 用户提供的 Linggan Intelligence 战略、能力、案例和数据底座讨论稿全部原件归档。

## 原因

目标已经从内容管理升级为长期情报系统。继续在旧 166 模型上演进会让页面、兼容关系和历史状态决定领域模型；全量复制又会把迁移胶水变成永久架构。

## 代价

- 已验证 TypeScript 内核需要按合同逐步移植到 Rust，不能声称代码复用等于功能完成。
- 首期速度低于直接复制旧仓库。
- 每个边界都需要跨语言 fixture 与真实 PostgreSQL 对照证明。

## 防幻觉验收

- Rust 结果与固定 TypeScript fixture 的 canonical JSON/hash 完全一致。
- 任一成功必须有实际数据库提交和端到端结果；mock/类型不计。
- 旧表、旧 migration、V1 fallback 在新运行仓库零命中。
