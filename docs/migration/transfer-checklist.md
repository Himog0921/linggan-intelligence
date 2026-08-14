# 跨电脑搬迁清单

## 搬运

- [ ] 本 Git 仓库，包含完整 Git 历史。
- [ ] 单独加密的 legacy PostgreSQL dump（可选）。
- [ ] 单独归档的 68GB 媒体（可选，建议先生成索引）。
- [ ] 新电脑重新配置的 `.env` 和密码；不要复制现役明文秘密。
- [ ] 插件 v2.0.95 源码快照和构建说明已包含在 `references/`。

## 不搬运

- [ ] 旧 `node_modules`、`.next`、`dist`、日志。
- [ ] 旧工作台完整工作目录。
- [ ] Chrome Profile、Cookie、IndexedDB 授权状态。
- [ ] GitHub production secrets、runtime DSN 文件。
- [ ] 旧数据库表进入新数据库。

## 新电脑验收

- [ ] `shasum -a 256 -c references/SOURCE-MANIFEST.sha256` 全部通过。
- [ ] `cargo test --workspace` 实际运行通过。
- [ ] PostgreSQL 16 可用，目标数据库名正确。
- [ ] 新数据库无旧表、无旧 migration ledger。
- [ ] legacy archive（若恢复）使用不同数据库名和只读账号。
- [ ] 浏览器插件在新环境重新授权，不复用旧 Cookie/Token。
