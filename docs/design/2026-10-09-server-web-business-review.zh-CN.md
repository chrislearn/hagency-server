# Server 网页业务与界面复核

日期：2026-10-09。范围：`crates/frontend` 的 Dioxus 网页，在本地受信任的
`https://hagency.local` 实际登录和截图检查。此次未修改后端业务模型、迁移数据库，
也未把浏览器变成本地 Agent executor。

## 当前业务边界

| 身份或入口 | 应提供的能力 | 不应混淆的权限 |
| --- | --- | --- |
| 普通用户：Projects | 查看自己所属的已登记 Space 和已加入的已登记 Room，查看 Agent 名单；有管理权时编辑接入政策和服务暂停 | 加入 Space 不等于加入其私密 Room；普通成员不可编辑政策 |
| 普通用户：My agents | 自己的服务器级 Agent、已分配执行设备、私聊及 Project Room binding、服务器可用性及 binding 暂停/恢复 | Agent 不隶属于某个 Project；服务器 binding 状态不等于本地设备正在运行 |
| Account center | 个人资料、安全、登录设备及会话 | 人员账号由 Pasion 管理，不把 Agent 傀儡身份作为人员登录账号 |
| Server Admin | Matrix 身份/Room 管理、审核、媒体、联邦、Appservice，以及 Pasion 账号/角色/会话/通知管理 | Admin 角色不自动获得别人的 Agent 所有权，也不代替 Space/Room 的实时 Matrix 管理权 |
| Hagency Desktop | 创建 Project/Agent、聊天、默认私聊、指定当前执行设备及后续转移、模型/推理强度/本地资源、额度和本地响应服务 | 网页内服务器暂停/恢复不负责启动用户设备上的服务 |

## 本次修正

- Projects 和 Room 选择器以实际名称展示；原始 ID 收入高级信息。登记已有 Space/Room 的表单折叠，避免误认为必须手工输入 ID 才能使用日常业务。
- 接入政策从服务状态读取当前 revision，按服务端实时 `canManagePolicy` 显示可编辑或只读状态。切换 Project/Room 会重建编辑器，避免旧草稿或异步响应串到别的对象；解析失败时禁止保存。
- 明确接入政策决定谁能添加自己的 Agent，服务暂停才会停止既有 binding。Project 暂停不能靠解除单个 Room 暂停绕过；解除暂停后仍需 owner 恢复 binding。
- My agents 展示执行设备及服务器身份，区分服务器暂停与本地运行。暂停/恢复使用一个随状态变化的按钮和图标；默认私聊不提供 Leave Room。添加已有 Agent 使用 Project/Room 选择器。
- 顶部显示 Administrator/Personal account，普通用户也可进入 Account center。键盘快捷键、OAuth 返回路径和拒绝访问页面按角色限制。
- 人员账号管理文案不再宣称只允许管理员登录；Matrix 用户列表区分人员、Agent 和 Hagency service。Agent 详情不再提供不存在的 Pasion 人员账号入口。
- 必需的内置 `hagency_agents_v1` Appservice 标为 Built in / Deployment managed，移除该条目的常规禁用、删除、覆盖入口；外部桥接管理保留。该保护仅限网页入口，不宣称改变了后台 API 的管理员权限。
- Auth Status 正确使用已认证接口读取版本，结合 issuer/discovery 判断 OIDC，避免将 Matrix 广告的协议登录 flow 误解释为人员原生密码登录可用。
- 统一标题/段落间距、按钮文字和图标居中、页面宽度与空状态；补齐缺失图标，保留键盘可见焦点。截图中的选择器焦点框是键盘操作状态。

## 实机验证

使用三个已有本地测试账号：Alice 是 Project/Room 管理者，Bob 是普通成员及 Demo assistant owner，demo-admin 是服务器管理员。

| 验证 | 结果 |
| --- | --- |
| Alice 查看 Launch demo / Team discussion | 显示名称、Room 和 Agent 名单；可以编辑接入政策 |
| Alice 保存相同 Project 政策两次 | 成功；自动重新读取 revision，实际 revision 从 1 到 3，权限内容不变 |
| Bob 查看同一 Project | 只读表单，无保存政策或服务暂停操作 |
| Alice / demo-admin 的 My agents | 正确为空，没有泄露 Bob 的 Agent 所有权 |
| Bob 的 My agents | 显示 Demo assistant、Hagency Desktop 执行设备、私聊和 Project Room binding；没有把 active binding 称为设备在线 |
| 添加已有 Agent 的高级表单 | 选择 Project 后才加载 Room；未选择 Room 时不可提交，没有实际添加重复 binding |
| Alice 直接访问 `/users` | 拒绝访问，仍能进入自己的 Projects/My agents |
| 管理员 Matrix Agent 身份详情 | 识别 Agent，隐藏 Pasion 人员账号入口 |
| 管理员 Account center / 人员账号详情 | 可打开，人员资料、角色、安全入口保留 |
| 管理员刷新 Auth Status | 经现有授权流程后返回原页面，未错误回到 Dashboard |

22 个管理员主菜单页面逐项打开并截图：Dashboard、Users、Rooms、Reports、Server Notices、Registration Tokens、Auth Status、Appservices、Local Accounts、Federation、Media、Audit Log、OAuth2 Sessions、Personal Tokens、Upstream Providers、Upstream Links、Notification Templates、Connector Health、Notification Channels、Notification Preferences、Projects、My agents。页面未出现接口错误或布局覆盖。

这是实际登录、读取、权限差异、布局和政策保存验证；没有执行删除账号、升降管理员角色、重置密码、发送服务器通知、发布模板、安装桥接或暂停正在使用的 Agent 等操作。空列表的功能未据此宣称完成全部写入流程验收。

自动检查：前端 14 项测试通过；后端 BrowserAuth 2 项测试通过；WASM 前端构建与 `git diff --check` 通过。权限回归覆盖实时身份/角色及封闭 BFF 路由；前端回归覆盖政策数据有效性和 OAuth 返回地址授权。

## 截图

完整原始截图位于 [截图目录](screenshots/2026-10-09-web-business-review/)。

- [管理员总览](screenshots/2026-10-09-web-business-review/admin-dashboard.png)
- [内置 Appservice](screenshots/2026-10-09-web-business-review/admin-appservices.png)
- [人员与 Agent 身份](screenshots/2026-10-09-web-business-review/admin-users.png)
- [Alice 的 Project](screenshots/2026-10-09-web-business-review/member-project-access.png)
- [Bob 的只读政策](screenshots/2026-10-09-web-business-review/member-readonly-access.png)
- [Bob 的 Agent](screenshots/2026-10-09-web-business-review/member-agents.png)
- [Agent 的聊天绑定](screenshots/2026-10-09-web-business-review/member-agent-service.png)
- [普通用户访问管理页面被拦截](screenshots/2026-10-09-web-business-review/member-admin-blocked.png)

`admin-review-1.jpg` 至 `admin-review-5.jpg` 是管理员截图的检查拼图；`before-*` 保存本次修改前的界面。辅助浏览器状态文本、账号密码与构建日志保留在忽略的本地 `.run`，未写入本报告。

## 部署及未完成项

新静态资源已更新到当前本地服务器；后端容器未重启。已生成本地镜像
`hagency-server:web-business-20261009`，用于后续重建容器时保留本次前端。
镜像基于现有 `room-pause-notice-20261008`，此次只覆盖前端静态资源。

网页登录的重复授权和刷新后的会话续期仍需改进：实测管理员升级管理 scope 或刷新时可能再次显示 Pasion 授权页；普通用户的一次刷新也回到了登录入口，SSO 可复用但仍出现授权页。当前 OAuth 凭据保存在内存中，刷新会丢失；不能据此把所有现象的根因都归于同一问题。本次修正了授权后的返回路径，但没有宣称消除重复授权或完成持久会话续期。

此次主要按系统深色主题检查桌面浏览器；没有完成独立的浅色主题、手机窄屏和所有管理写入操作验收。加密 Room 中的 Agent 执行仍受现有客户端 crypto 支持范围限制。
