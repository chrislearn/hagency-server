# Hagency Operations

[English](README.md) · [架构与迁移](../../docs/OPERATIONS.zh-CN.md)

`hagency-operations` 负责 Hagency 业务工作流，是由 `crates/backend` 挂载的
Rust/Salvo 库，不需要另部署服务器。它依赖 `hagency-contract`、PostgreSQL/Diesel
和 Matrix HTTP API，不依赖 Palpo 内部 crate、Node.js 或 SQLite。

提供原生客户端 Matrix 身份会话、项目/Agent/额度追加审批、Inbox、按角色读取、
不可变命令、持久化回执、通知意图及 Matrix 提醒 worker。与现有管理接口共享
hagency 数据库、状态文档和 fleet 投递队列。

原生入口为 `/_hagency/miniapp/v1/`，兼容旧 `/_palpo/miniapp/v1/` 和 `palpo.*`
服务名。网页通过已有 cookie/CSRF 保护的 `POST /api/operations/call` 调用同一
业务逻辑；Inbox 页面是 `/hagency/inbox`。

审批通过不代表执行完成，也不代表 Agent 已能聊天。执行回执、当前观察与运行端
预算预留分别验证。新授权投影通过显式离线命令导入；导入并不自动建立真实授权证明。

运行 `just check-operations`。PostgreSQL 重启测试必须使用空的专用测试数据库：

```sh
HAGENCY_TEST_DATABASE_URL=postgres://... cargo test --locked -p hagency-operations \
  postgres_shared_writer -- --ignored
```

当前边界和后续运行端、客户端验收要求见[架构与迁移](../../docs/OPERATIONS.zh-CN.md)。
