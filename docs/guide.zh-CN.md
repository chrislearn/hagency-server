# 配置与开发指南

[English](guide.md) · [快速配置](../README.zh-CN.md) · [文档目录](README.zh-CN.md)

Palpo Matrix homeserver、Pasion 认证和 Palpo web-admin 的功能运行在**同一个 Rust 进程、同一个 HTTP 监听端口**中。
后端使用 Rust、Tokio、Salvo、Diesel/diesel-async 和 PostgreSQL，与 Palpo 技术栈一致。生产环境不运行 Node 服务器。

Cargo workspace 采用类似 Pasion 的后端、前端组织方式：

```text
crates/
├─ backend/   # hagency-server：内嵌 Palpo、Pasion 和 Hagency API
└─ frontend/  # hagency-frontend：基于复制的 Padmin 源码，使用 Dioxus/WASM
xtask/       # 开发、配置工具（workspace 成员）
resources/   # 生成的前端与 Pasion 资源，不提交到 Git
```

前端将 Padmin 的 Matrix 管理页面与原 web-admin 的项目、Agent 资源选择和申请、提供方授权和配对、Agent 身份、兼容模式账号审批及审计功能结合。
Rust 后端在 `/` 托管前端；Matrix 路径为 `/_matrix`，Palpo 管理 API 为 `/_palpo`，Matrix 发现路径为 `/.well-known/matrix`，Hagency API 为 `/api`，Pasion 挂载在 `/_pasion/`。
`/healthz` 表示进程 HTTP 服务可访问，不代表异步账号同步已经完成。

## 目录

- [架构](#架构)
- [开发环境](#开发环境)
- [统一前端认证](#统一前端认证)
- [组件配置与首次管理员](#组件配置与首次管理员)
- [Compose 部署](#compose-部署)
- [保留的业务行为与限制](#保留的业务行为与限制)
- [验证方法](#验证方法)
- [内嵌 Pasion](#内嵌-pasion)
- [数据库迁移](#拆分旧的合并数据库)
- [配置迁移](#将旧的单一配置拆成组件配置)

以下命令均在仓库根目录执行。

## 架构

```text
hagency-server（一个进程、一个端口）
├─ MatrixServer：Palpo 初始化、Matrix/管理/发现路由及后台任务
├─ Rust web-admin：会话、Fleet/Agent、项目申请、兼容模式账号审批
├─ outbound relay：持久化事务、租约、ACK 和已发布快照
├─ PasionServer：/_pasion/ 下的 OAuth/OIDC、账号界面/API 和后台任务
└─ 集成 Padmin + Hagency 的 Dioxus/WASM 静态资源
           │
           └─ PostgreSQL 服务
              ├─ hagency：管理状态和持久化 outbound 队列
              ├─ palpo：Matrix homeserver 数据
              └─ pasion：认证数据
```

Palpo 以 Rust 库形式链接，不作为独立进程启动，也不依赖外部反向代理连接。
Matrix 授权和 App Service 操作仍由 Palpo 的 API 验证。web-admin 的 Rust 适配层通过固定的 loopback 地址调用**同一个进程**挂载的 Matrix/管理 API，以保留原有授权语义。
内部地址从 `listen` 推导，公开传输地址从 `public_origin` 推导。

Palpo 当前配置与连接池是全局单例，因此每个进程只允许一个 MatrixServer。
宿主负责 Tokio、tracing、HTTP 关闭流程和 TLS 终止方式；二进制将 Tokio 工作线程栈设置为 8 MiB。
Matrix 后台任务随运行时停止。默认 Compose 的 HTTP 服务由前置 HTTPS 代理提供 TLS。

## 开发环境

依赖 Rust ≥ 1.99、[just](https://github.com/casey/just)、PostgreSQL 客户端库 `libpq`，以及用于启动 PostgreSQL 的 Docker。
Pasion 资源构建还需要 `wasm32-unknown-unknown` 目标和 Dioxus 0.7.5 资源；准备工具会在需要时下载匹配的 Dioxus CLI。
Git 和 curl 用于获取工具或源码。两个前端都通过 Rust 和 Dioxus CLI 构建，不需要 Node.js 或 npm。
只有执行 `tests/` 下 JavaScript HTTP/集成测试脚本时才需要可选的 Node.js，不需要 Python。
`rust-toolchain.toml` 固定 Rust 1.99.0，并配置 rustfmt、Clippy 和 WASM 目标；Docker 构建阶段使用对应的 Rust 1.99 镜像。

`just --list` 显示全部命令。Just 负责组织命令；workspace 中的 Rust `xtask` 实现私有配置生成、资源准备和开发监听，运行这些工具不需要先编译服务器。

```sh
just init-dev
just db-up
just prepare-pasion
just prepare-frontend
# 新数据库需要先按后文说明创建管理员。
just dev
```

打开 `http://127.0.0.1:8088`。开发配置的固定 Matrix 服务器名是 `localhost:8088`。
后端 Rust 修改编译成功后会优雅重启；编译失败时保留上一次正常运行的服务器。
前端 Rust/CSS 修改会重新生成 WASM 资源，运行中的宿主直接读取这些资源，刷新浏览器即可生效。仅前端修改不会重启后端，不需要重新构建 Docker 镜像。

浏览器 access/refresh token 只保存在内存中，因此整页刷新后需要重新登录，SPA 页面跳转会保留登录状态。
宿主重启后，尚在浏览器内存中的 Matrix token 可以重新绑定到新的 Hagency cookie 会话。

开发 Palpo 源码时，使用具有 MatrixServer API 的本地仓库：

```sh
just dev --palpo-source /absolute/path/to/palpo-checkout
```

开发工具在忽略的 `.run/` 下生成本地 Cargo patch，监听 Palpo 源码变化并重新编译，不修改上游仓库，也不需要重建 Docker 镜像。
在固定 Git 依赖和本地 patch 之间切换时，Cargo 可能更新锁文件；移除覆盖后可用 `cargo update -p palpo` 恢复普通依赖。

复制的 Padmin 还包含可选 `palpo_admin` sidecar 的维护、定时命令和通知页面。
这个独立 sidecar 未内嵌，因此对应菜单保持禁用。Matrix 用户、房间、媒体、注册和 App Service 管理直接使用挂载的 Palpo API。

## 统一前端认证

Pasion 是默认身份提供方。宿主通过 `/config.json` 发布非敏感运行配置，前端只有一个 **Sign in with Pasion** 入口，使用 Authorization Code + PKCE。
前端读取已登录 Pasion 账号的角色，仅为管理员申请管理 scope。
全新浏览器中的管理员登录会先确认身份，再为同一个账号继续申请管理权限；普通成员不会获得管理 scope。

Pasion 管理密码、注册、账号状态和管理员角色，后台 provisioning 任务将对应 Matrix 身份和管理员标记同步到 Palpo。
在 `/pasion/accounts` 管理人员账号，`/users` 查看 Matrix 记录和服务身份。修改密码和创建人员账号走 Pasion。
Hagency 保存项目、提供方、申请状态和业务权限，不建立另一套密码账号库。

宿主创建预留的公开 OAuth 客户端，并自动推导 `/oauth/callback`，无需客户端密钥或单独部署前端。
`POST /api/login/token` 验证 token 后发放 HttpOnly 会话 cookie 和 CSRF token。
Palpo 管理接口还在宿主边界检查 Pasion 实时 introspection 与精确的 `urn:palpo:admin:*` scope；即使账号是管理员，仅有成员 scope 的 token 也无法管理服务器。
关闭 introspection 缓存后，撤销角色或 token 会在下一次请求生效。Pasion 自身的管理 API 也检查 scope 和当前角色。

如需原生 Matrix 认证兼容模式，可在 Pasion 配置中设置 `[hagency].delegate_matrix_auth = false`。
该模式使用 Palpo 原生密码登录和原 web-admin 的 Matrix 账号审批流程，默认统一部署不使用它。

前端来源和上游许可证信息见根目录 `NOTICE`。
复制的 Padmin 基于提交 `83d4567470ada808b89914aa0c786cdc3a7ac89a`。

## 客户端 Pasion 登录与自助接入

`hagency-client` 可以通过 Pasion 登录后自动创建自己的 Fleet，保存配置并启动 outbound 连接，无需手工下载和导入。首次账号绑定仍需本地访问权限；后续登录固定为同一服务器和账号。

在 Hagency 自己的配置中开放自助接入（默认关闭）：

```toml
[fleet_access]
allow_self_service = true
max_per_user = 3
```

管理员只需开放政策，无需逐次批准普通用户的接入。每个本地安装通过固定安装 ID 幂等创建自己的 Fleet，每个 Fleet 对应一个 App Service；注册权限和 Pasion 共享密钥留在服务器。关闭自助接入会阻止新接入，不会撤销已经配置的连接。

原生接口位于 `/_hagency/client/v1/`：公开 discovery；identity 验证 Pasion 用户 token；fleets 创建并返回本人的配置；fleets/{id}/connect 自动验证接入。认证接口只接受原生 Bearer 请求，不接受浏览器 Cookie/Origin。创建只接受 installationId/name，所有者由验证后的身份决定，不授予 Matrix 管理员权限。

客户端的人类会话最长 15 分钟，最多每 30 秒复验一次 token，重启后重新登录。机器连接凭据独立于浏览器登录，token 撤销会终止本地登录访问，但不会自动撤销 Fleet。停用 Fleet 使用服务器的连接管理。

## 组件配置与首次管理员

配置由各组件分别持有：

```text
config/
├─ examples/       # 提交到 Git 的原生配置模板
│  ├─ hagency.toml
│  ├─ palpo.toml
│  └─ pasion.toml
├─ dev/            # 生成的开发配置，Git 忽略，文件权限 0600
│  ├─ hagency.toml
│  ├─ palpo.toml
│  └─ pasion.toml
└─ docker/         # 生成的部署配置，Git 忽略，文件权限 0600
   ├─ hagency.toml
   ├─ palpo.toml
   └─ pasion.toml
```

启动参数为 `--config config/dev/hagency.toml`，也是 CLI 默认值。
Hagency 管理监听、管理连接、数据目录，并引用其余两个文件：

```toml
palpo_config = "palpo.toml"
pasion_config = "pasion.toml"
```

Palpo 使用原生顶层 ServerConfig，包括 `[db]`、`[storage]`、`[well_known]`，不套 `[matrix]`。
Pasion 使用原生 `[database]`、`[account]`、`[email]`、`[[clients]]`、`[[upstream_oauth2.providers]]` 等段，不套 `[pasion.settings]`。
额外的 `[hagency]` 段保存内嵌选项 `resources_dir` 和 `delegate_matrix_auth`。

| 配置项 | 数据库 | 内容 |
| --- | --- | --- |
| `hagency.toml` 的 `database_url` | `hagency` | web-admin/Fleet/项目状态和 outbound 队列 |
| `palpo.toml` 的 `db.url` | `palpo` | Matrix 用户、房间、事件和 homeserver 状态 |
| `pasion.toml` 的 `database.uri` | `pasion` | 账号、OAuth/OIDC token 和会话 |

数据库名必须不同。省略 `pasion_config` 可关闭 Pasion，改用独立的 Hagency、Palpo 两个数据库。
组件引用、媒体路径及支持的原生密钥文件引用，均相对于声明它们的配置文件解析。
开发监听会监控所有三个文件，也包括主配置目录之外的引用文件。

`--check-config` 加载并检查全部引用文件，不启动服务器；`--list-config-files` 输出路径 JSON，不连接数据库。
内嵌模式下使用 Hagency 的监听配置，覆盖 Palpo/Pasion 的监听配置。
如果 `palpo.toml` 没有显式 `[keypair]`，首次生成 Matrix 签名密钥，保存为 `data_dir/matrix-signing-key.json`，权限 0600。
数据目录与 PostgreSQL 应一同备份。`server_name` 是 Matrix 身份的一部分，复用数据库时不能改变。

默认关闭注册。为新数据库创建第一个管理员时，先把强密码放进权限受保护的 `secrets/admin-password` 文件，再启动应用：

```sh
# 将强密码保存到受保护的 secrets/admin-password 文件。
just run --config config/dev/hagency.toml --bootstrap-admin admin \
  --bootstrap-password-file secrets/admin-password
```

默认 Pasion 委托模式下，bootstrap 按 Pasion 密码策略创建首个管理员，排队执行正常的 Matrix 身份及角色同步。
通过 Pasion 使用 `admin` 登录，Matrix 身份是 `@admin:localhost:8088`。
bootstrap 不会覆盖现有 Pasion 账号；已有 Pasion 管理员时也会拒绝执行。后续管理员角色在账号管理中授予。

对于以前使用原生 Matrix 认证初始化的部署，可显式关联现有活跃人员管理员，使用相同用户名和选定的 Pasion 密码文件：

```sh
just run --config config/dev/hagency.toml --bootstrap-admin admin \
  --bootstrap-password-file secrets/admin-password --link-existing-matrix-admin
```

关联会保留 Matrix ID、房间和 Hagency 所有权。访客、服务账号、暂停、停用或非管理员身份不能关联。
其他原生账号及密码不会自动导入；开启委托前应先准备或迁移对应 Pasion 账号。
原密码文件满足 Pasion 策略时可以复用。执行 bootstrap 前停止开发监听，确保一个数据库只对应一个服务器进程；后续启动省略全部 bootstrap 参数。

可选 `account_config` 接受原 web-admin JSON 字段：`botMxid`、`botToken`、`adminToken`、`approvers`、`passwordKey`、`registrationToken`。
兼容模式 worker 验证私有邀请制管理员房间、精确审批事件和审批人的当前权限，再创建普通 Matrix 账号。
待处理密码用 AES-256-GCM 加密，公开 API、审计记录和审批卡片不包含密码或 access token。
此可选流程需要开启 Palpo 注册并配置对应 registration token。
`retirement_admin_token_file` 可单独指定 Hagency 最终配额退役操作使用的服务器端凭据文件。

## Compose 部署

```sh
just init-docker --origin https://palpo.instance \
  --server-name palpo.instance
just db-up
just docker-build
```

这会创建 `config/docker/{hagency,palpo,pasion}.toml`，以及保存随机数据库密码的受保护 `.env`。
开发生成器对应创建 `config/dev/`；`--output-dir` 可指定其他目录。生成器拒绝覆盖已有目录。
首个命令只准备配置，后两个准备数据库和镜像，供下文创建管理员使用。
Compose 部署 PostgreSQL 与单个 hagency-server 进程。Docker 构建使用一个编译任务，并降低大型 Palpo crate 的优化等级以减少峰值内存；其加密、HTTP 和数据库依赖仍保持完整优化。

把仅暴露在 loopback 上的 HTTP 端口放在 HTTPS 代理后，保留公开 Host 头。
将 `/`、`/api`、`/_matrix`、`/_palpo`、`/_pasion/`、`/.well-known/matrix` 和 `/healthz` 转发到 8088。
浏览器、管理和 outbound 客户端使用公开 origin；Palpo App Service relay 使用自动推导的内部 origin。
无需为 web-admin 单独配置 origin、端口或 relay URL。

Compose 把 `config/docker/` 挂载到 `/app/config/`。
入口脚本先复制配置树到 `/run/hagency/config/`，目录权限 0700、文件权限 0600，再切换到 UID 10001。
组件文件之间及目录内密钥文件的相对引用保持有效，Rust 进程以非 root 用户运行。
生成的生产路径是绝对路径；可选挂载的账号配置和 token 文件也应使用绝对路径，并允许 UID 10001 读取。

在 PostgreSQL 健康后创建首个管理员：

```sh
docker compose run --rm --service-ports \
  -v "$PWD/secrets/admin-password:/app/bootstrap-password:ro" server \
  --config /app/config/hagency.toml --bootstrap-admin admin \
  --bootstrap-password-file /run/hagency/bootstrap-password
```

账号创建后停止这个初始进程，使用 `docker compose up -d server` 正常启动。
已初始化的部署可使用 `docker compose up -d --build`。
bootstrap 拒绝覆盖已有账号，不应在镜像或配置里放置共享默认管理员密码。

## 保留的业务行为与限制

- Matrix 密码登录；不透明 HttpOnly/SameSite cookie、CSRF、Origin/Host 检查、请求体上限、限流、实时身份/管理员检查及超时。
- Fleet 授权、验证安装、冲突/漂移检查、仅 owner 可获取的凭据、暂停/恢复/最终撤销和审计。
- 受管理 Agent 身份创建、受限的资料修改、真实成员关系检查及退役验证，包括 Hagency 最终配额退役。
- 精确 Matrix probe 回执、绑定 generation 的 outbound 连接证明、项目及私有审批房间、权限检查、绑定来源的申请和真实准入/状态观测。
- 持久化 outbound 事务、顺序 lane、租约、ACK tombstone、容量限制、凭据轮换、迁移和重放、不可变更新序列、当前快照，以及拒绝过期/未知 readiness。
- 可选兼容模式账号审批 worker：私有回执、加密待处理密码、认证审批结果、注册恢复和结果通知。

PostgreSQL 文档状态和投递队列原子提交。专用连接上的会话级 advisory lock 在连接存续期间限制单写者。
数据库故障会阻止写入，恢复该连接需要重启进程。
当前文档存储沿用原单服务器、单写者模式，队列有容量上限，不是集群调度系统。

当前整合 Palpo、Pasion、Padmin 和 Hagency web 管理；以项目为中心的 Rinx 客户端是后续工作。
旧 SQLite 管理数据不会自动导入；单独验证迁移方案前，这一版本应使用新的 PostgreSQL 管理数据库。

## 验证方法

执行下面的 `tests/*.mjs` 脚本需要安装 Node.js；前端构建、本地服务器开发和部署不需要它。

```sh
just check-tools
cargo fmt --check
cargo check --all-targets --locked
cargo test --locked
cargo build --example admin_contract_server --locked
cargo test --locked -p hagency-frontend --target <native-host-triple>
node tests/http-contract.mjs
node tests/native-client-contract.mjs
# Real client/server enrollment with controlled Pasion/Matrix peers:
CONTRACT_SERVER=/absolute/path/to/admin_contract_server \
HAGENCY_CLIENT=/absolute/path/to/hagency \
CONSOLE_ASSETS=/absolute/path/to/client-console-assets \
node tests/client-enrollment.e2e.mjs
# 如果编译产物位于其他目录：
CONTRACT_SERVER=/absolute/path/to/admin_contract_server node tests/http-contract.mjs
```

将 `<native-host-triple>` 替换为 `rustc -vV` 输出的 `host`，例如 `aarch64-apple-darwin`。
运行 HTTP 契约测试前先构建前端资源。

HTTP 契约测试会启动 Rust 适配层及受控 Matrix fixture，覆盖 Dioxus SPA/资源和 token 桥接、认证隔离、Fleet/Agent 生命周期、项目准入、outbound proof/ACK/轮换、精确请求绑定、最终配额退役和获批账号创建，不连接模型。
PostgreSQL 持久化/锁测试必须显式提供专用 `HAGENCY_TEST_DATABASE_URL`：

```sh
HAGENCY_TEST_DATABASE_URL=postgres://... cargo test --lib postgres_restart -- --ignored
```

`tests/integration.mjs` 使用专用 Hagency、Palpo 数据库验证真实二进制，包括管理员登录、Matrix 发现、Fleet 注册、真实 relay 投递、重启恢复和签名密钥持久化。
任一数据库已有应用表时，脚本会拒绝运行。安装 `psql`、创建两个空的专用数据库后运行：

```sh
HAGENCY_TEST_DATABASE_URL=postgres://.../hagency \
PALPO_TEST_DATABASE_URL=postgres://.../palpo node tests/integration.mjs
# 构建镜像并验证隔离部署与重启：
docker build -t hagency-server:integration-check .
node tests/docker-smoke.mjs
```

统一 Pasion 账号与 PKCE 集成测试需要三个**空的**专用数据库和 Pasion 资源：

```sh
HAGENCY_TEST_DATABASE_URL=postgres://.../hagency_test \
PALPO_TEST_DATABASE_URL=postgres://.../palpo_test \
PASION_TEST_DATABASE_URL=postgres://.../pasion_test node tests/pasion-integration.mjs
```

若服务器二进制不在 `target/debug/`，设置 `HAGENCY_BINARY`；Pasion 资源不在 `resources/pasion/` 时，设置 `PASION_TEST_RESOURCES`。
这些脚本会写入专用测试数据库，再次运行时使用新的空数据库。

Docker smoke 测试创建唯一 Compose 项目，完成后删除自己创建的容器和卷。
已执行的检查和限制见[验证记录](VALIDATION.zh-CN.md)。
Palpo 内嵌改动见[上游 PR #505](https://github.com/palpo-im/palpo/pull/505)。

### 内嵌 Pasion

Pasion 与 Palpo、web-admin 共用 Rust 进程和端口。
内嵌 API 及子路径支持见[Pasion PR #102](https://github.com/meldry-com/pasion/pull/102)。

| 组件 | URL |
| --- | --- |
| 整合的管理前端 | `/` |
| Matrix 客户端/联邦/管理 API | 原 Matrix/Palpo 路径 |
| Pasion 账号界面 | `/_pasion/`、`/_pasion/login` |
| OIDC 发现 | `/_pasion/.well-known/openid-configuration` |
| OIDC issuer | `<public_origin>/_pasion/` |
| OAuth token / JWKS | `/_pasion/oauth2/token`、`/_pasion/oauth2/keys.json` |

无需单独的 Pasion daemon、公开 IP、Node 服务器或代理路径重写。
Pasion 后台任务通过同一监听端口的 loopback 地址调用受保护的 Palpo MAS API。
宿主生成并持久化 OAuth 签名密钥、cookie 加密密钥和 Matrix 共享密钥，保存在权限 0600 的 `data/pasion-secrets.json`，应随数据库一起备份。

三个组件在同一个 PostgreSQL 服务中使用**独立数据库**。
Compose 通过 `POSTGRES_DB` 创建 `hagency`，首次初始化新卷时 `deploy/databases.sql` 创建 `palpo` 和 `pasion`。
已有卷不会重新执行初始化脚本。升级旧的 Hagency/Palpo 合并数据库前，请先完成[数据库拆分](#拆分旧的合并数据库)。

在 `hagency.toml` 用 `pasion_config` 引用 `pasion.toml` 即可启用内嵌，生成的配置默认包含该引用。
Pasion 数据库配置放在自身文件的 `[database].uri`，资源目录放在 `[hagency].resources_dir`。
原生邮件、SMS、账号注册、客户端、上游 OAuth 提供方、连接池上限、限流和品牌设置直接写在 `pasion.toml`。
网络/issuer、Matrix 连接、模板、存储及密钥归属由 Hagency 推导，这些段不能覆盖宿主连接设置。
原生数据库连接池和 TLS 设置会保留，三个数据库依然独立。

本地开发时先构建一次 Pasion Dioxus WASM 前端及资源：

```sh
just prepare-pasion
just dev --pasion-source /path/to/pasion --palpo-source /path/to/palpo
```

`just prepare-pasion` 默认使用固定版本源码，`--source` 可指定本地仓库。
命令会在需要时安装 Rust `wasm32-unknown-unknown` 目标。
工具复用 `dx` 0.7.5，或下载并校验匹配版本到 `.run/tools/`，不替换全局已安装版本。
开发监听会重新构建修改后的 Pasion 后端/前端，复制模板、翻译和策略，再优雅替换服务器，不需要重建镜像。
Docker 镜像构建过程会构建并打包这些资源。

本地注册测试先启用 `[account].password_registration_enabled`，再在 `config/dev/pasion.toml` 设置：

```toml
[experimental]
fixed_verification_code = "123456"
[email.provider]
type = "blackhole"
[sms.provider]
type = "blackhole"
```

邮件、SMS 联系方式验证会保存 `123456`，不发送验证通知。
在验证页面填写该值即可；若注册开始于设置启用之前，先重新发送一次验证码。
开发监听会重载配置。本地测试之外不要设置固定验证码。
可用 `[account].password_registration_contact_required = false` 取消注册时必填联系方式。

生成的配置默认启用 Pasion 委托。宿主自动连接 Palpo 发现、token introspection 和兼容的 Matrix 密码登录。
`/_pasion/` 是账号中心，`/_pasion/register` 是注册页，`/login` 是整合管理界面的登录入口。
统一模式下 `/account-request` 重定向到 Pasion 注册，旧的 Hagency 账号审批菜单隐藏。
人员账号审批应在 Pasion 注册/访问策略中实现；旧审批 worker 保留在显式原生认证模式，不能和委托注册一起使用。

Pasion 不实现旧 Matrix SSO 重定向。OAuth 客户端使用挂载的 issuer，旧 Matrix 密码客户端走委托密码登录。
保留的业务流程和认证改动见[web-admin 功能对照](WEB_ADMIN_PARITY.zh-CN.md)。

项目清单声明 Apache-2.0，项目仅保留一个许可证文件 `LICENSE`。
`NOTICE` 保留 Palpo、Padmin、Pasion 的来源与上游许可证链接。
Padmin 源码和 Pasion 依赖仍保留其上游 AGPL 许可；修改项目清单不会重新许可这些代码。

### 拆分旧的合并数据库

不会自动重命名数据库、搬迁数据或改写私有配置。
如果旧部署将 Matrix 和管理表放在同一个 `hagency` 数据库，先停止所有服务器，并备份数据库与持久化密钥/媒体目录。
以下命令假定原 Compose 部署尚无 `palpo` 数据库，且存在 `public.hagency_admin_state` 表；连接这些数据库的本地原生服务器也必须先停止。

```sh
docker compose stop server
umask 077
mkdir -p backups/db-split
docker compose exec -T postgres pg_dump -U hagency -Fc hagency > backups/db-split/hagency-before-split.dump
docker compose exec -T postgres pg_dump -U hagency --no-owner --no-privileges -t public.hagency_admin_state hagency > backups/db-split/admin.sql
# 任一备份失败时，停止执行后续命令。
docker compose exec -T postgres psql -U hagency -d postgres -v ON_ERROR_STOP=1 \
  -c 'ALTER DATABASE hagency RENAME TO palpo;' \
  -c 'CREATE DATABASE hagency OWNER hagency;'
docker compose exec -T postgres psql -U hagency -d hagency --single-transaction -v ON_ERROR_STOP=1 < backups/db-split/admin.sql
```

将 `hagency.toml` 的 `database_url` 指向 `/hagency`，`palpo.toml` 的 `[db].url` 指向 `/palpo`，`pasion.toml` 的 `[database].uri` 指向 `/pasion`。
保留原 Matrix 服务器名和密钥/媒体目录。此前未启用 Pasion 时，先创建一次对应数据库再启用。
配置生成器不会覆盖已有文件。

确认管理表恢复成功后，从 `palpo` 删除原管理表，使其只保存 Matrix 数据：

```sh
docker compose exec -T postgres psql -U hagency -d palpo -v ON_ERROR_STOP=1 -c 'DROP TABLE public.hagency_admin_state;'
```

之后以更新后的配置和镜像重启。
数据库重命名和管理表复制保留 Matrix 账号、Fleet 和队列状态，Pasion 数据保留在自身数据库。

### 将旧的单一配置拆成组件配置

已有三个独立数据库的部署，在拆分配置时保留数据库 URL、服务器身份、密钥/媒体路径及认证设置：

1. 把宿主设置放进 `hagency.toml`，用 `palpo_config` 和可选 `pasion_config` 文件引用替代内联组件段。
2. 将 `[matrix]` 内容移到 `palpo.toml` 顶层，去除子段的 `matrix.` 前缀，例如 `[matrix.db]` 改为 `[db]`。
3. 将 `[pasion].database_url` 改为 `pasion.toml` 中的 `[database].uri`；`resources_dir`、`delegate_matrix_auth` 移入 `[hagency]`；展开 `[pasion.settings.*]` 到对应原生段。
4. 按新文件位置调整相对路径，或继续使用绝对路径；通过 `hagency-server --config <path>/hagency.toml --check-config` 检查。

旧的私有 `config.dev.toml`、`config.docker.toml` 不会自动改写或导入，迁移期间应保留。
如果数据库仍合并，先完成上面的数据库拆分，再启用新配置。
