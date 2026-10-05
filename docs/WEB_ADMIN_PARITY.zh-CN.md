# Palpo web-admin 迁移与统一账号

[English](WEB_ADMIN_PARITY.md) · [文档目录](README.zh-CN.md)

对照基线为本地 Palpo `web-admin` 前端重构提交 `1032153e`；实际内嵌 Palpo 版本记录在根目录 `Cargo.toml`。
迁移目标是在 Padmin 的 Dioxus 界面中提供等价的 Hagency 业务流程，不要求 HTML、DOM 选择器或 Node 服务完全相同。
默认部署将账号认证统一到 Pasion。

| 业务流程 | Rust/Dioxus 实现 | 验证方式 |
| --- | --- | --- |
| 提供方授权、隔离 App Service、可恢复安装、漂移检测 | 后端 admin/fleet.rs；前端 Fleet connections | HTTP 契约；真实 Matrix 集成 |
| Outbound 迁移/轮换、队列限制、租约、ACK、在线与当前 generation 证明 | 后端 admin/outbound.rs；连接及容量检查 | HTTP 契约；真实 Matrix 集成 |
| 仅 owner 可配对/下载配置、精确事件回执及 reception 成员关系 | 后端 admin/workflow.rs；My Fleets | HTTP 契约；真实 Matrix 集成 |
| 创建项目或绑定已有房间、加密私有审批房间、owner/member 权限 | 后端 admin/workflow.rs；Projects | HTTP 契约；真实 Matrix 集成 |
| 选择发布的资源和角色、Agent 命名、token/每日配额、持久化申请/重试 | 后端 admin/workflow.rs；Request an agent | HTTP 契约；前端角色/资源检查 |
| 提供方决策、真实运行配置和准备阶段、过期状态、真正入房后才可用 | 后端 admin/workflow.rs；Agent requests | HTTP 契约；真实 Matrix 集成；就绪分组回归 |
| 受管理身份、受限改名、退役时 Matrix 停用/成员关系检查 | 后端 admin/fleet.rs；Agent identities | HTTP 契约 |
| 审计、owner 隔离、凭据脱敏、Origin/CSRF/实时角色检查 | 后端 admin/api.rs | HTTP 契约 |
| Callback proof 续期、一分钟重试间隔、展示错误并阻止提交 | Hagency common.rs 及页面提示 | 源码对照；编译；浏览器检查 |
| 原生账号申请/回执、私有 Matrix 管理员房间审批 | 后端 admin/accounts.rs；Hagency approvals | 仅显式原生认证模式保留；HTTP 契约 |

申请页面展示 `provider.serving` 的实际 framework、model、reasoning 和 tier，fulfillment 阶段/错误、投递与审批说明、过期状态及原来的 attention/review/usable 分组。
活跃但不可用的 Agent 不会进入 Ready to use。资源卡片可直接填入 Agent 定义。
Callback 策略可见，未配置允许地址时对应选项禁用。续期错误在重试间隔内持续显示。

## 身份系统的有意调整

默认 `[hagency].delegate_matrix_auth = true`，由 Pasion 管理注册、密码、账号状态和管理员角色，既有 provisioning 任务同步 Matrix 身份及角色。
整合界面只有一个 Pasion 登录入口；成员申请客户端/设备 scope，管理员在确认身份后继续为同一账号授权管理 scope。

初步成员授权暂存在宿主的 HttpOnly 会话中，待替换授权验证成功后，宿主直接在 Pasion 撤销初步 OAuth grant，保留当前 Matrix 设备。
这样无需再次输入密码，成功切换后也不留下仍有效的初步授权。
前端 bearer/refresh token 仍只保存在内存中。

内嵌 Palpo 的管理中间件检查本地 Matrix 角色；宿主补充实时 Pasion introspection 和精确 Palpo 管理 scope 检查。
边界层拒绝原生人员账号创建和密码写入，仍允许 Matrix 资料更新及 App Service 身份。
Pasion API 自身验证管理角色和 scope。撤销在下一次请求生效，没有 introspection 缓存宽限期。
创建人员账号、重置密码和角色变更走 Pasion 账号管理；此模式下隐藏原生用户 CSV 导入。

统一部署中，旧的 Matrix 注册/Robrix 审批表单不再形成第二套注册系统：`/account-request` 重定向到 Pasion 注册，旧审批菜单隐藏。
可选旧审批业务保留在兼容模式，尚未重新实现为 Pasion 审批策略。
如果部署必须保留该精确的注册前审批策略，应先补充 Pasion 策略/工作流集成，再替换旧注册流程。

首个管理员 CLI 创建 Pasion 账号，通过其 password manager 散列密码，持有 bootstrap 锁，并排队执行正常 provisioning。
显式 `--link-existing-matrix-admin` 可保留已有活跃人员管理员的 Matrix ID。
bootstrap 不会覆盖已有 Pasion 账号或管理员。

Padmin 可选的 `palpo_admin` 运维 sidecar 不属于 Palpo `web-admin`，不在本次迁移内，其菜单继续禁用。
