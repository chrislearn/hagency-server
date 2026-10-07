# Palpo/Pasion 管理与新的 Agent 边界

[English](WEB_ADMIN_PARITY.md) · [文档目录](README.zh-CN.md)

统一 Dioxus 控制台保留 Palpo/Pasion 的通用管理功能。旧 Hagency Fleet/Engagement
审批产品已被替换，没有兼容模式。上游依赖版本和前端复制来源仍记录在 Cargo.toml 与 NOTICE。

| 能力 | 当前实现与边界 |
| --- | --- |
| Matrix 用户/设备、Room/Space/成员/state、媒体、举报、destinations | 原通用前端页面直接调用 Palpo 自有 API |
| Appservice、registration token、服务器状态/操作/通知 | 原 Palpo 管理页面和 API 授权保留 |
| 人员注册/密码/账号状态/管理员角色 | Pasion 账号中心、管理 API 与正常 provisioning |
| OAuth/个人会话、上游 provider/link、审计、connector、提醒 | 原 Pasion 管理页面和 Pasion 自有授权 |
| 个人 Pasion PKCE、cookie/CSRF、实时管理员检查、grant handover | 独立 `backend/browser_auth.rs`，无旧业务 store |
| 永久 Agent owner、Project=Space、独立 Room binding、创建政策 | 新 PG `agent-service/domain` 与 Projects/My agents 页面 |
| 必装 Appservice、durable AS ACK、owner 队列、执行租约 fencing、回复 outbox | 新 `agent-service` 与可信 Matrix adapter/workers |
| 本地 Codex/模型、Room/user 配额、请求过滤、高风险工具决策 | 用户的 `chrislearn/hagency-client`，无需服务器人员审批 |
| 加密 Room Agent 执行 | 等待客户端 crypto，不宣称已经可用 |

已删除 Fleet/Hafleet enrollment/配对/每安装独立 Appservice、provider 资源选择/
allocation 审批、Engagement/delegation import、私有账号审批 Room、旧 requests/
inbox 页面、miniapp aliases 与原生密码界面。旧代码、配置和兼容别名不再属于当前版本。
详见 [切除实施边界](../crates/agent-service/legacy-cutover.md)。

Pasion 管理人员身份并将 Matrix 账号/角色同步到 Palpo。用户获得成员 scope，经过
真实管理员检查才申请额外管理 scope。Palpo 管理接口继续检查本地 Matrix 角色与宿主的
实时 Pasion 管理 scope；Pasion API 检查自身角色/scope。现有 delegated-auth 边界
拒绝人员原生密码/账号写入，Matrix profile/service identity 保留正常接口。

首次管理员 CLI 保留 Pasion password hashing、bootstrap lock 与正常 provisioning。
显式 `--link-existing-matrix-admin` 可关联已有的活跃人员管理员，保留 Matrix 身份；
不导入 Fleet 状态、不覆盖已有 Pasion 账号。

网页 BFF 只允许封闭 Project/Agent/binding 管理操作，不是通用代理或设备 executor。
body 不能指定 owner、Matrix 成员事实或管理员证明。Project 创建政策由真实 Space
管理权治理；服务器管理员身份本身不授予 Agent 所有权。Padmin 可选的独立
`palpo_admin` sidecar 仍不属于本宿主。

真实执行证据记录在 VALIDATION。静态编译与受控 BrowserAuth 回归不能单独证明完整
浏览器视觉验收或真实 Matrix/PKCE 投递成功。
