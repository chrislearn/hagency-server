# Appservice 领域代码草稿

本目录保存 2026-10-07 误建于 `hagency-org/hagency-server` 的独立 Rust 领域代码。正确服务端项目为 `/Volumes/Data/Works/chrislearn/hagency-server`；本草稿已移入该仓库，但尚未集成至现有 backend、operations 或 PostgreSQL 存储。

它不是第二个服务端产品，也不是已完成的重构。现有服务端根 Cargo workspace 和 README 保持原状。草稿使用 SQLite，与真实服务端 PostgreSQL 存储不同，不能直接作为正式存储方案；整合前须按真实仓库结构重新设计。

迁移前 `cargo test --offline` 的 10 个领域测试通过，覆盖 owner 不可变、跨 Project 绑定、创建许可与运行许可区分、幂等性及旧数据库拒绝。尚无 HTTP、真实身份校验、Appservice 收发或端到端执行验证。

实施报告：[客户端报告](../../../hagency-client/docs/design/2026-10-07-server-appservice-client-refactor.zh-CN.md)。
