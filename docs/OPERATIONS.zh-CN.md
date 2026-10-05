# Hagency 业务归属与迁移

[English](OPERATIONS.md) · [文档目录](README.zh-CN.md)

## Fleet 命名

Fleet 是一套资源服务在本服务器的接入注册，归属于 Pasion/Matrix 用户，拥有
独立的 App Service、凭据、命名空间、资源目录和连接生命周期。一个 Fleet 可
提供多个 Agent、服务多个项目。界面为 **My Fleets**，入口为
`/hagency/fleets`；旧 `/hagency/hafleets`、`/hagency/my-hagencys` 书签自动跳转。
Rust 模块、函数和标识符统一使用 `fleet` / `Fleet`。

管理客户端使用 `/api/fleets`、`/api/my/fleets`，标准响应使用 `fleet`、
`fleets`、`fleetId`、`fleetName`。原生接入使用 `/_hagency/client/v1/fleets`
及 `fleets/{id}/connect`；自助接入政策位于 `[fleet_access]`。
旧 Hafleet URL、输入字段 `hafleetId` 和配置键 `[hafleet_access]` 仍可使用。
旧管理/原生接入 URL 的响应另带旧字段别名；输入同时包含不同值的
`hafleetId` / `fleetId` 时，在创建操作前拒绝。

机器接口与下载凭据使用 `/api/fleet/v2/{id}`；旧 `/api/hafleet/v2/{id}` 保留
同样的认证校验。既有 `hf_` ID、App Service 注册、Matrix 事件绑定、规范摘要
正文、数据库键和审计历史保留。此次改名无需轮换 token、重新注册或迁移数据库。

## 组件边界

| 组件 | 负责 | 不承担 |
| --- | --- | --- |
| Palpo / MatrixServer | Matrix 客户端与联邦协议、用户身份校验、房间、事件、App Service 和 Matrix 服务器管理 API | Hagency 项目、资源、coordinator、预算、Inbox、Agent 审批 |
| Pasion | 账号、OIDC/OAuth、会话、Matrix token 认证和账号角色 | Hagency 资源授权、配额分配 |
| `crates/hagency-contract` | server engagement、项目/Agent/追加额度协议、策略、预算运算、规范摘要 | HTTP 认证、数据库事务、实际额度预留 |
| `crates/operations` | 原生客户端接入、coordinator 审批、Inbox、执行回执投影、通知、持久化工作流 | Codex/Claude 实际调用与运行端额度账本 |
| `crates/backend` | 一个监听端口挂载上述能力；fleet 配对、连接证明、房间准备、账号衔接和现有管理 API | 另起 Node.js Operations 服务 |
| `crates/frontend` / Rinx | 网页管理与原生项目管理客户端 | 通过界面声明取得审批权限 |
| hagency-rs | 资源所有权、实际额度预留、Agent 创建、Codex/Claude 执行、用量和执行回执 | 从 Matrix 管理员身份推断资源授权 |

项目仍使用一个服务器和三个数据库：hagency、palpo、pasion。
Operations 与原管理功能共享 hagency 的 `public.hagency_admin_state` JSONB 文档，
不新增 SQLite。决定、审计、命令 outbox、通知意图与投递记录共用一次持久化提交。
克隆的 Store 共用写锁，进程之间通过原有 PostgreSQL advisory lock 排他；
HTTP 超时或取消不会中断已开始的提交。

## 本次迁移范围

来源固定为 Palpo PR #508 的 `985c7242c2074b7cb0561c14c7c79dc6ed1a2bf7`。
保留并迁入原协议和 HTTP 工作流测试，SQLite 专用测试改为 PostgreSQL 验证。
迁移包含 #506 的客户端/Inbox 方向与 #508 的 coordinator 协议；
#507 的旧“指定 Matrix 管理员审批项目”模型不引入第二套权限系统。
角色模型依据 [Rinx ADR 0011](https://github.com/hagency-org/Rinx/blob/main/docs/adr/0011-hagency-server-engagements.md)。

| 能力 | 当前落点 |
| --- | --- |
| 规范 JSON 摘要、有限预算运算、不可变类型及审批校验 | `hagency-contract` |
| 借用 Matrix 身份、15 分钟原生会话、能力协商与撤销 | `operations::api` / `matrix` |
| 项目、Agent、额度追加申请，审批/拒绝、seen/snooze | `operations::workflow` / `intents` |
| 按角色分页读取、未知/陈旧用量、审批与执行状态分离 | `operations::views` / `updates` |
| durable outbox、代际/修订校验、精确重试、执行回执 | `operations::workflow` / `outbound` / `updates` |
| 私有 My Actions 通知房间、幂等消息、有限提醒和重试 | `operations::notifications`，可选 worker |
| 网页 Inbox | `/hagency/inbox`，Padmin 其它页面保留 |
| 网页与原生共用业务逻辑 | `POST /api/operations/call` |
| 原有 fleet 注册、配对、连接、relay/poll/ACK、retirement | 原 backend 管理模块，继续使用共享存储 |

`hagency-contract` 和 `hagency-operations` 均不依赖 Palpo 内部 crate。
主 backend 嵌入 Palpo 只为提供 MatrixServer。这里不会运行 Palpo 的 Node web-admin，
也不会加载其 Operations SQLite 数据库。已提交 Palpo 清理草稿
[PR #512](https://github.com/palpo-im/palpo/pull/512)，移除旧 Node web-admin 和专用 CI，
保留通用 Matrix/App Service 管理 API。上游 main 尚未删除旧应用；替代版本发布、
消费者切换和既有状态迁移验收后再合并。#508 的业务 crate 尚未合入 Palpo main。

## 身份、权限与执行

Pasion 登录得到 Matrix access token。网页沿用现有 cookie、同源和 CSRF 检查；
原生客户端以 Matrix bearer 建立短时会话，每次调用再次校验 Matrix 身份。
原生接口拒绝 Origin/Cookie 浏览器上下文，网页通过专用 adapter 调用。

Matrix 管理员权限不等于 coordinator 权限。可信授权投影指定资源所有者和
coordinator；项目审批由 coordinator 进行，Agent/追加额度还允许政策授权的
资源所有者审批。自主审批必须明确允许。不同 server engagement 即使位于同一
homeserver，也不能共享授权；单 Agent allocation 的旧 engagementId 不可混用。
当前投递映射要求 serverEngagementId 与它所使用的 fleet/profile ID 对应。

一个审批写入决定与命令，不代表已执行。传输 ACK 仅确认接收；执行回执与当前
运行观察再推进 `pending`、`provisioning`、`ready`。运行端必须在真实账本事务内
再次验证授权和父级容量，并预留额度；服务器不能用页面数字制造可用额度。
未上报用量保持 unknown，旧观察保持 stale，追加额度不能自动增大父级 grant。

## 接口兼容

新原生入口：

```text
POST /_hagency/miniapp/v1/session
POST /_hagency/miniapp/v1/call
POST /_hagency/miniapp/v1/disconnect
appId: im.hagency.operations
services: hagency.inbox.*, hagency.projects.list, hagency.requests.list,
          hagency.intent.new, hagency.session.open, hagency.session.disconnect
```

兼容 `/_palpo/miniapp/v1/`、`im.palpo.operations` 与旧 `palpo.*` 服务名，
供 Rinx 渐进切换。命令 wire version、摘要和序列化 ID 不因 crate 改名变化。
只授予已实现服务；旧 manifest 中尚未迁入的服务不会假装可用。
网页 adapter 的请求为 `{ "service": "hagency.inbox.list", "args": { "view": "all" } }`；
写操作沿用 expectedRevision 和稳定 commandId，客户端不能选择认证 actor。

## 授权投影导入

新关联审批、资源所有者在线签发 delegation 和客户端房间准备，在上游 #508 中
尚未完成。本次迁移不把旧管理员、旧请求或 hostname 自动提升为 coordinator。
现阶段由操作员离线导入经审核的投影，服务器必须先停止：

```sh
just import-authority /absolute/path/reviewed-authority.json
# 指定其它配置：
just import-authority /absolute/path/reviewed-authority.json config/docker/hagency.toml
```

命令使用 hagency 数据库，校验 homeserver、ID、修订和关联，只导入后退出，
不启动 Palpo/Pasion。运行中会因 advisory lock 拒绝导入。
JSON 含 `engagements`、`resources`、`projects` 三个 map；schema 示例见
[工作流测试](../crates/operations/tests/workflows.rs)。测试 fixture 不是真实授权证明。
不得通过删除再添加记录重置授权历史；撤销/移除使用 tombstone 和递增修订。

已存在的 Postgres 文档、凭据、旧请求、投递租约及未知扩展字段保持原样。
新业务记录保存在 `rustWorkflows`。这是增量保存，不是旧 Inbox 的语义自动转换，
也不是 Node SQLite 的自动导入工具。既有连接继续原工作流；新 coordinator 流程
需明确授权与支持对应协议的 hagency-rs，并单独验收。

## 可选 Matrix 提醒

在 `hagency.toml` 添加：

```toml
[action_notifications]
bot_mxid = "@notifications:palpo.instance"
token_file = "../../secrets/notifications-token"
```

这是普通 Pasion/Matrix 账号的 access token；不另设业务管理员账号，也不需要
管理员 token。文件路径相对 hagency.toml；令牌应受文件权限保护。配置省略时
worker 不启动，Inbox 仍可用。修改令牌后重启服务器。

worker 通过 Matrix API 创建 private、禁止联邦的 My Actions 房间，只允许 bot
与收件人加入/受邀，逐次验证绑定、历史可见性和 state 权限。消息只有通用文字
和不透明 action 引用，不带定义、凭据或审批指令。需要进入 Inbox 完成审批。
首次通知后最多三次提醒（1 小时、1 天、2 天）；seen/snooze、角色变化和新修订
抑制旧提醒。丢失响应以同一 Matrix transaction ID 重试，错误采用有限退避。

## 验收与后续

```sh
just check-operations
cargo test --locked -p hagency-server
cargo check --locked -p hagency-frontend --target wasm32-unknown-unknown
```

PostgreSQL 重启/并发测试使用空的专用 `HAGENCY_TEST_DATABASE_URL`，执行
`cargo test --locked -p hagency-operations postgres_shared_writer -- --ignored`。

后续运行端从本仓库引用 `hagency-contract`，Rinx 使用新命名空间。完整在线关联、
delegation 签发、旧数据逐项对账、真实 hagency-rs 创建/聊天与 Codex/Claude 配额
执行仍需跨项目验收。不能把通过本地 fixture 测试当成这些运行流程已完成。
Palpo PR #504 的 URL 预览属于 Matrix 配置，应继续留在 Palpo；只在本项目文档
解释白名单，不迁入 Hagency 业务模块。
