# Hagency 业务协议

[English](README.md) · [架构与迁移](../../docs/OPERATIONS.zh-CN.md)

`hagency-contract` 由 hagency-server 维护，定义 server engagement、项目授权、
Agent 分配、额度追加、版本/代际绑定、规范 JSON 摘要、审批策略和预算计算。
它不依赖 Palpo 内部 crate，仅通过 Ruma 校验标准 Matrix 标识。

服务器负责认证用户并在持久化事务内记录决定；hagency-rs 在真实额度预留事务
中再次校验授权、修订、摘要和容量。此 crate 本身不会认证、预留额度或创建 Agent。
Matrix 管理员不自动获得 coordinator 业务权限；资源所有者授权的 coordinator
审批项目和 Agent，允许的资源所有者也能审批 Agent。自主审批需要明确授权。

共享协议保留上游 wire version、序列化 ID 和摘要算法；旧的单 Agent allocation
engagement ID 与 server engagement ID 仍是不同概念。运行端以后应从
`chrislearn/hagency-server` 引用此 crate，不能继续将业务协议归属 Palpo。

运行 `cargo test --locked -p hagency-contract`。
