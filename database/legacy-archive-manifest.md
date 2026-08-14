# Legacy 数据归档清单

## 数据库备份

当前记录的最近完整备份：

- 原始位置：`/Users/gongyong/Services/content-workbench/migration/database-backups/pre-migrate-20260813T115321Z-b3e02cbafc89.dump`
- 格式：PostgreSQL custom-format dump
- 大小：689,809,710 bytes（约 658MiB）
- SHA-256：`9a2aa0f9976a5ffe32b805acf603f3c553570a24fc473a3ff467ca27e8f1ed12`
- 状态：未复制进新项目

此备份包含旧系统数据，只能单独加密搬运并恢复为只读 `linggan_legacy_archive`。它不是新项目 seed，也不能提交 Git。

## 本地媒体

- 原始位置：`/Users/gongyong/Services/content-workbench/migration/app/storage/local-blob`
- 盘点体积：约 68GB
- 状态：未复制进新项目

后续应生成对象索引与分片校验值，再决定哪些高价值媒体进入独立归档盘。不得把整个目录放入源码项目。

## 新电脑恢复原则

1. 新项目数据库保持为空并只运行新 baseline migration。
2. 如确需历史查询，将 dump 恢复到单独数据库。
3. legacy 账号只读；新 API/worker 配置禁止出现 legacy DSN。
4. 新事实通过插件重新观察进入新 Evidence 链。
