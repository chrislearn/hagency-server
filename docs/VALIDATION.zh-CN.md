# 已执行的验证

[English](VALIDATION.md) · [文档目录](README.zh-CN.md)

这是早期实现的按时间记录。当前行为以最后的统一 Pasion 章节和[配置指南](guide.zh-CN.md)为准；
前面关于可选委托认证、旧前端的说明已被后续章节取代。本次文档整理不会重新执行这些历史检查。

2026-10-03 的本地基线使用 Rust 1.98.1、PostgreSQL 18.6、Node 26.9。
当时固定的公共 Palpo Git 版本为 `c8568d9844a6be0a3172d98d7c9810e3a1f7521c`
（[PR #505](https://github.com/palpo-im/palpo/pull/505)）。

## 初始版本通过的检查

- Palpo 库/二进制编译及库测试：244 项通过，17 项按需数据库回归未启用；包括挂载路由所有权回归。
- `cargo fmt --check`、锁定依赖的全部 target 编译、`cargo clippy --all-targets --locked --no-deps -- -D warnings`。
- `cargo test --locked`：原子回滚和两项统一配置/签名密钥测试。
- 按需 PostgreSQL 测试：投递状态持久化、重开/重启、单写者 advisory lock、拒绝第二个写者。
- 十组受控 Matrix HTTP 契约：精确前端字节，会话/Host/Origin/CSRF/实时授权，owner 凭据隔离，Fleet 安装/幂等/生命周期，受管理身份，私有项目审批房间、申请来源绑定，outbound 租约/ACK/proof/轮换，资源准入和最终配额退役，以及私有认证审批后的普通账号创建。
- 真实集成二进制及空的隔离 PostgreSQL 数据库：Matrix 管理员创建/登录、首页、Matrix 版本/发现、真实 App Service 注册及代表身份创建、真实事件事务投递到挂载 relay、ACK、精确 probe proof、优雅重启，以及用户/Fleet/凭据/签名密钥/投递历史保留。
- 旧前端 HTML/JS/CSS 与 Palpo 源码逐字节一致。
- 开发配置 `--check-config`、当时 Python/Node/shell 语法和 Compose 配置检查。

## 初始 Docker 验证

完全优化的首次构建超出 Docker VM 的 8 GB 内存预算，后改为单编译任务，并对大型 Palpo crate 使用 opt-level 1，其依赖仍完整优化。

- Linux arm64 的 `docker build -t hagency-server:integration-check .` 通过，最终 release 构建在 8 GB 预算内完成。
- `node tests/docker-smoke.mjs` 通过：隔离 PostgreSQL/Rust 服务，PID 1 为 UID 10001，运行配置权限 0600，旧首页不变，Matrix API、真实管理员密码登录、优雅重启、账号保留、持久化签名密钥一致。
- 测试工具删除了自己创建的 Compose 容器和卷。

镜像 manifest：`sha256:acc80a21ba2a2b5772110463ec4faf1e4112609cd09f5c31a6e0d1c600d573d5`。
Docker 报告大小：359,121,160 字节。

## 范围和限制

受控契约 fixture 验证 HTTP 行为和状态变化，不验证真实 Codex/Claude 模型运行时。
真实集成验证 Palpo 事件投递和重启恢复，但不覆盖 Matrix 联邦、全部 Matrix API 或真实审批 bot 的完整可选账号流程。
这批历史测试没有停止、改配或迁移已有开发栈/用户数据库，测试容器和卷隔离并在完成后清理。

适配层保留单写者模型，浏览器会话仅在内存中，旧 SQLite 管理状态不会自动迁移。
Palpo 内嵌 API 是进程全局，每个运行时/进程只能有一个 MatrixServer。

## 上游 CI 基线

当时 Palpo main 的严格 workspace Clippy 存在 17 项来自既有 Diesel 衍生数据结构的 redundant-field 诊断：
[main workflow](https://github.com/palpo-im/palpo/actions/runs/37019841267)。
PR #505 的诊断相同，位于本次改动之外；与下游已通过的全部 target Clippy 检查分开记录。
库抽取还暴露了两个旧文档 import，已在 Palpo PR 的文档后续改动修复，`cargo test -p palpo --doc --locked` 两项通过。
宿主仍固定在实际完成原生/Docker 集成验证的运行版本，没有直接更新到未验证版本。

## 内嵌 Pasion（2026-10-04）

此阶段 Pasion 固定在 `b8333e1ee60c6362847d0f1890461b7af7547037`
（[PR #102](https://github.com/meldry-com/pasion/pull/102)），共用 Tokio/Salvo 进程和端口，挂载 `/_pasion/`，在同一个 PostgreSQL 服务中使用独立数据库。
当时没有修改已有开发部署。

执行并通过：

- Pasion 挂载状态隔离、SPA 前缀、API schema 基路径、旧账号重定向、loopback homeserver 不走代理，以及模板前缀测试。
- 独立 Pasion CLI/后端编译和 CI 固定版本 formatter。
- Dioxus 0.7.5 release WASM 资源使用 `--base-path /_pasion/` 构建。
- 浏览器登录和 Security Center 页面；带前缀的 JS/WASM/CSS 正常，无浏览器错误。
- 宿主配置回归：即使 host 写法不同，也会在写密钥或创建数据目录之前拒绝相同数据库名。
- 链接 Pasion 后，旧 web-admin 十组 HTTP 契约和真实 Palpo/Fleet 集成仍通过。
- 最终固定版本 `cargo build --locked`、`cargo test --all-targets --locked`：一项原子状态和三项配置测试通过，一项按需数据库测试未启用。
- 最终原生集成：同端口、前缀 discovery/SPA/JS/WASM/API schema、旧账号重定向、注册/浏览器登录、后台 Palpo provisioning、委托 Matrix 密码登录/introspection、重启后签名/加密密钥一致、优雅关闭。
- 最终宿主格式和严格全部 target Clippy。
- Linux arm64 Docker 构建在 8 GB 预算内完成，缓存依赖后的后端 release 用时 8 分 50 秒，Dioxus 资源也在独立构建阶段生成。
- `HAGENCY_TEST_IMAGE=hagency-server:pasion-integration-check node tests/docker-smoke.mjs`：旧首页、真实管理员登录、同端口 Matrix/Pasion discovery 与账号界面、UID 10001、0600 配置/OAuth 密钥、优雅重启、账号保留、Matrix 签名密钥和 OAuth JWKS 一致。容器和卷已清理。

Pasion 集成镜像 manifest：`sha256:a542c5c46cc2774fa3dedad5e1c382ac79ec6fefaebafc2cdbe787e8a4849496`。
Docker 报告大小：439,346,725 字节。原生测试 PostgreSQL 容器及临时数据也已清理。

这一阶段验证真实注册、后台 Palpo provisioning、委托密码登录和 introspection，没有覆盖全部 OIDC 授权码/PKCE 或外部提供方流程。
当时委托 Matrix 认证仍是可选项，已有 Palpo 密码不自动迁移到 Pasion。

上游 PR 的仅测试后续改动修复 discovery cache 隔离：同进程独立 AppState 之前复用了首个 issuer。
修复后完整后端 suite 在隔离 PostgreSQL 中 191 项通过、1 项忽略。宿主仍固定在前述已验证生产版本，这个后续改动不改变生产缓存行为。

## 三个独立数据库（2026-10-04）

前面的章节记录双数据库基线。本阶段新增独立的顶层 Hagency `database_url`，当时 Palpo 为 `matrix.db.url`，Pasion 为 `pasion.database_url`。
默认是同一个 PostgreSQL 服务中的 `hagency`、`palpo`、`pasion`。
新 Compose 卷创建三库；已有卷/配置需要手动完成文档中的拆分，不会自动更改。

执行并通过：

- 锁定依赖的原生二进制/example 编译、全部 target 测试、格式、严格 Clippy。数据库两两冲突（包括不同 host 写法）、缺少 Hagency URL、无效 PostgreSQL URL 在初始化前被拒绝。
- 在隔离临时目录生成开发/Compose 配置，用真实二进制验证三条连接 URL 和权限 0600。
- 原十组 HTTP 契约。
- 独立 Hagency/Palpo 库中的真实 Palpo/Fleet 集成：管理库仅有 `hagency_admin_state`，Matrix 库有 homeserver 表而无管理表；真实事件投递/ACK/proof 和重启后用户/Fleet/transport 状态保留。
- 三个独立库的真实 Pasion 集成：注册、后台 Palpo provisioning、委托 Matrix 登录/introspection、带前缀的前端/API/discovery、密钥稳定、优雅重启/关闭。
- Linux arm64 镜像和隔离 Compose smoke：三库对应表集合、非 root UID、0600 配置/密钥、旧首页、原生管理员登录、Matrix/Pasion 端点、重启后账号和签名密钥保留。

镜像 manifest：`sha256:78915ebed3329376292f5c5a7b16095bf364a66243aacfa3bca8a9932575b7bc`。
Docker 大小：439,336,859 字节。数据库/集成测试部署均用隔离容器和临时配置，没有改变已有开发栈或私有配置。

## 各组件独立配置文件（2026-10-04）

配置切换为 `config/dev/`、`config/docker/`，各含 `hagency.toml`、`palpo.toml`、`pasion.toml`，模板位于 `config/examples/`。
Hagency 引用组件文件，Palpo 保留原生 ServerConfig 段，Pasion 保留原生段和 `[hagency]` 内嵌选项，数据库名和运行归属未改变。

执行并通过：

- 锁定原生二进制/example 编译和全部 target 测试，含四项配置回归：密钥持久化、schema/Host 检查、三库冲突/缺 URL、独立组件文件相对路径解析（媒体、registration token、密码密钥文件）。
- 严格全部 target Clippy 和格式。
- 隔离目录的开发/生产生成器：三份有效文件、0600 文件/0700 目录、指定 origin/Matrix 身份、原生文件引用、拒绝覆盖已有输出。
- `--list-config-files` 不访问数据库即返回全部组件路径；开发监听覆盖主目录外的引用文件。
- 原生 Palpo 配置下的十组 HTTP 契约。
- 独立宿主/Palpo 配置及数据库的真实 Palpo/Fleet 投递/ACK/proof 和重启集成。
- 三配置文件下的真实 Pasion 注册、Palpo provisioning、委托登录/token introspection、前缀界面/API/discovery、重启密钥稳定；原生数据库连接池设置保留。
- 新开发文件仅生成和检查，当时未启动应用或修改已有数据库，旧根目录私有配置保留。
- 最终 Linux arm64 镜像和配置目录挂载 Compose smoke：三份原生配置全部加载、UID 10001 下 0700 运行目录/0600 文件、旧前端、真实管理员登录、Matrix/Pasion 端点、独立库表、优雅重启和相同 Matrix/OAuth 签名密钥。测试容器和卷已删除。

镜像 manifest：`sha256:c6efb58bc46828c5464a4a87b1442a3dd9014618b05f36c6e611508c17ad5ea7`。
Docker 大小：439,287,638 字节。原生测试 PostgreSQL 也已删除，没有重启或迁移当时已有应用栈/数据库。

## Just 命令与 Rust 开发工具（2026-10-04）

三个 Python 工具替换为独立 Rust `xtask` 和根目录 `justfile`。
配置路径/schema、服务器二进制和前端行为不变。Docker 资源准备也使用 Rust 工具，不安装 Python。

执行并通过：

- 当时 `just check-tools`：格式、严格全部 target Clippy 和四项测试，覆盖私有配置权限、密码 URI 转义、独立数据库名、保留既有凭据/配置、本地资源复制和 Dioxus 安装前校验。
- 隔离编译器/服务器 fixture 验证 watcher：编译或配置错误保留运行进程；恢复、外部 Palpo/Pasion 配置修改、本地 Palpo 源码修改触发替换；SIGTERM 优雅停止子进程。支持自定义 Cargo 产物目录和含空格的路径。
- 真实 just recipe 生成临时三文件开发配置，真实服务器 `--check-config` 验证；`just prepare-pasion --skip-frontend` 复制本地资源，含空格参数完整传递，既有私有配置保留。
- Just/服务器格式、diff 空白、Compose 配置检查。
- 使用 Rust 资源工具的 Linux arm64 镜像构建及真实 Dioxus WASM 构建。隔离 Compose smoke：三组件配置/独立数据库、非 root、同端口 web-admin/Matrix/Pasion、管理员登录、稳定密钥和重启。测试容器/卷已删除，既有服务未重启。

镜像 manifest：`sha256:35c0a571c3f33607d2ef6187aa35a67a420e50fdd9f57ea2246c0c324bae6b6d`。
Docker 大小：439,287,638 字节。最终运行镜像不含 Python 和 Rust 开发辅助工具。

## 后端/前端 workspace 与集成 Padmin（2026-10-04）

本节取代之前“原 HTML/JS 前端不变”的说明。
宿主改为虚拟 Cargo workspace 的 `crates/backend`；`crates/frontend` 复制 Padmin 源码并加入原生 Dioxus Hagency 页面。
这一阶段 Padmin 来源记录在 NOTICE，保留原 AGPL 许可证。

执行并通过：

- 后端格式、严格全部 target Clippy、六项单元/配置测试。专用 PostgreSQL 单写者测试仍按需启用，真实重启和库隔离由集成脚本覆盖。
- Dioxus/WASM release 构建和十项原生前端测试：成员/管理员 OAuth scope 隔离、带端口或 IPv6 的 Matrix ID、已发布资源角色聚合。
- `just check-tools`：格式、严格 Clippy、CLI、私有配置生成、本地 Pasion 资源、watcher 恢复及外部组件配置监听；并发前端构建串行化，防止 Dioxus 覆盖另一个构建的工作文件。
- 十一组 HTTP 契约：SPA 路径、WASM MIME/magic、运行配置、保留路由隔离、路径穿越拒绝、token-to-cookie 桥接、实时管理员检查、Origin/CSRF、owner 隔离、callback/outbound 配对、项目/申请、账号审批、退役、持久投递和凭据撤销。
- 两个独立空库的真实 Palpo 集成：管理员登录、App Service 安装、真实 relay/proof 投递、优雅重启后队列与签名密钥保留。
- 三个独立空库的真实 Pasion 集成：挂载 discovery/SPA/API、注册/登录、委托 Matrix 登录/introspection、预留前端客户端和运行设置、成员 token 桥接、密钥稳定、优雅重启，临时库已清理。
- 原生开发服务器和隔离 OAuth 服务器分别使用独立浏览器会话：原生管理员登录、Dashboard、Users、Rooms、Hagency connections、项目、申请、审批无未捕获 JS 错误；重启后内存 Matrix token 成功绑定到新 Hagency cookie，修正后的用户列表保留完整 `@admin:localhost:8088`。
- OAuth 成员登录进入 Projects，仅有服务导航，`/api/fleets` 为 403；当时在隔离 Pasion 和 Palpo 数据库同时授予管理员的账号完成管理 OAuth 流程，打开 Pasion 账号列表，未影响开发账号。

这一阶段的限制：可选 `palpo_admin` sidecar 运维页面保留源码，未内嵌所以菜单禁用；当时 Pasion/Matrix 管理员角色仍分开。
整页刷新需要登录，因为 bearer/refresh token 在内存中，SPA 跳转不需要。

最终镜像也通过独立 Compose 的 `tests/docker-smoke.mjs`：三独立数据库、原生 Matrix 管理员登录、Rust 托管 Dioxus index/SPA/JS/WASM、运行设置、Matrix/Pasion API、UID 10001、0600 配置/密钥、优雅重启与签名密钥持久化。
临时容器/卷已清理。本地开发入口为 `http://127.0.0.1:8088/`，Pasion 账号界面为 `/_pasion/`。

## 统一 Pasion 账号与 web-admin 功能补齐（2026-10-04）

本节取代前面的独立角色、默认原生认证说明。
生成的配置默认启用 Pasion 委托，由 Pasion 管理人员密码、注册和管理员角色，provisioning 队列同步 Matrix 身份/角色。
界面只有一个 Pasion 登录入口。业务对照及旧审批边界见[功能对照](WEB_ADMIN_PARITY.zh-CN.md)。

执行并通过：

- 后端格式、严格全部 target Clippy、七项单元/配置测试；可选专用 PostgreSQL 锁测试仍按需启用。
- 十二项原生前端测试和 release Dioxus/WASM 构建，包括绑定同一身份的管理员授权，以及“活跃但不可用的申请不能进入 Ready to use”的回归。
- 全部十一组受控 HTTP 契约，保留 callback/outbound 配对、授权、proof、持久投递、项目/申请和兼容模式账号审批。
- 三个全新隔离数据库的真实 Pasion 集成：首个管理员 bootstrap、实际 Authorization Code + PKCE/consent、即使管理员账号也拒绝成员 scope 管理、拒绝人员创建/密码绕过、Matrix 资料更新、Pasion 授权/撤销同步、既有 Matrix token 和 Hagency cookie 立即失去管理权限、重启后签名密钥稳定。授权切换撤销初步 grant，同时保留替代授权的 Matrix 设备/token。临时库已删除。
- 全新独立浏览器会话的管理员/成员登录：管理员 scope 升级不再要求第二次密码，打开 Dashboard、Hagency connections、Pasion Accounts；未配置允许 origin 时 callback 禁用。成员进入 Projects，仅有服务导航。均无未捕获 JS 错误。
- 最终 Linux arm64 镜像的隔离 Compose smoke：三组件数据库、Pasion 管理员 bootstrap 和真实 OAuth 登录、非 root、受保护配置/密钥、托管 JS/WASM/SPA、discovery、Matrix API、重启和 Matrix/Pasion 密钥保留。健康接口就绪早于异步用户创建，因此等待管理员 Matrix 记录后才执行授权测试。测试容器/卷已删除。
- `just check-tools`：格式、严格 Clippy、全部五项测试通过。

切换认证前已私有备份本地开发数据库。
已有 `@admin:localhost:8088` 显式关联为首个 Pasion 管理员，保留 Matrix ID 和原密码文件，Pasion 登录通过；既有 Pasion 成员保留。
私有配置、备份、凭据均由 Git 忽略。正常启动使用 `just dev`，不带 bootstrap 参数。

旧的 Matrix 房间注册前审批策略仅保留在显式原生认证兼容模式。
统一部署使用 Pasion 注册和账号管理，该可选旧策略尚未在 Pasion 内重新实现。
前端 bearer/refresh token 仍在内存中，整页刷新需要重新登录。

## 双语文档、许可证声明与 Rust 1.99（2026-10-04）

README 仅保留中英文快速配置。详细指南、功能对照和历史验证记录统一放在 `docs`，均有中英文和互相跳转链接。
中英文 shell/TOML 示例一致，已检查本地 Markdown 路径、标题锚点和代码围栏。

三个项目包都声明 Apache-2.0 和 Rust 1.99，项目唯一许可证文件为 `LICENSE`。
NOTICE 保留 Palpo/Padmin/Pasion 的来源和原许可，这次元数据修改不会重新许可上游 AGPL 代码。
前面“保留上游许可证文件”等说明记录的是这次编辑之前的历史状态。

Rust 1.99.0 下执行的检查：

- 锁定依赖的 workspace/全部 target `cargo check`、WASM 前端 `cargo check`、workspace 格式检查通过。前端既有 dead-code 警告仍存在。
- `just check-tools` 的格式、严格 Clippy、全部五项测试通过。
- Cargo metadata 确认 backend、frontend、xtask 都为 Apache-2.0、Rust 1.99。
- 确认 Rust 1.99 bookworm/trixie Docker manifest 标签可用；本次元数据/文档修改未重新构建完整镜像。
- 已有本地服务器健康检查仍正常；本次文档和工具链调整没有修改数据库或私有配置。

## Node.js 依赖说明修正（2026-10-04）

禁用 Node.js/npm/npx 命令后，`just prepare-frontend` 和 `just prepare-pasion` 均完成真实 release/WASM 打包，没有调用 Node 工具。
已修正两版 README 的依赖清单及详细指南，删除 Docker web-tools 阶段中未使用的 `nodejs` 安装。
Node.js 仅用于 `tests/*.mjs`，前端构建和服务器运行不需要它。本次删除依赖未重新构建完整 Docker 镜像。

## Hagency 协议与 Operations 迁移（2026-10-05）

将 Palpo 草稿 #508 的 `985c7242c2074b7cb0561c14c7c79dc6ed1a2bf7` 中已实现的协议与 Operations 工作流迁入本项目 workspace。
Operations 和原 fleet API 共用现有 Hagency PostgreSQL 状态与写锁；没有增加第四个业务数据库或 SQLite 服务。
归属、兼容方式及剩余整合边界见 [Operations 说明](OPERATIONS.zh-CN.md)。

执行的验证：

- 协议/Operations：40 项测试通过。可选 PostgreSQL 测试也在独立空库中通过，覆盖竞争审批、回滚、进程排他、重启、精确重试，以及旧扩展字段和投递租约保留；临时测试库已删除。
- 后端：八项单元/配置测试通过；原有可选 PostgreSQL 回归本次未重跑。全部十二组 HTTP 契约通过，包含网页 adapter、原生路径兼容和既有 fleet 接口。
- 协议、Operations 和后端的格式及全部 target 严格 Clippy 通过。xtask 五项测试通过；开发监视已包含两个新 crate。
- WASM 前端检查和生产打包通过。前端仍有 49 项既有 unused/dead-code 警告；macOS 后端链接器仍提示既有的大 `__eh_frame` 警告。
- 本地开发进程已更新，`/healthz` 正常。现有 Pasion 管理员登录后可进入 Dashboard，Padmin 导航保留；`/hagency/inbox` 成功加载空列表。
- 通知测试使用本机 Matrix stub，覆盖私有房间校验、丢失响应的 transaction 去重、仅元数据通知和 seen/snooze。开发配置未启用可选通知 worker，没有发送真实通知消息。

本次未重建 Docker 镜像，也未运行真实 hagency-rs delegation、Agent 创建/聊天或 Codex/Claude 配额执行。
Palpo 清理在独立分支提交为草稿 [PR #512](https://github.com/palpo-im/palpo/pull/512)，移除旧应用及其 CI，补充双语迁移说明。
确认全部 Rust 源码、Cargo 清单/锁文件、Matrix 测试/部署均未修改，没有遗留的应用路径引用，空白检查通过；这种源码移除/文档变更未重跑 Palpo Rust 测试。
替代版本发布及客户端/状态迁移验收前，该 PR 保持草稿。草稿中尚未完成的在线关联/delegation 不声明为已实现。

## Fleet 名称统一（2026-10-05）

- hagency-client 与 hagency-server 的界面、源码、原生接入接口、配置示例及
  中英文文档统一为 Fleet。
- Workspace 库测试 8 项通过，Operations HTTP 工作流 16 项通过；两项依赖
  专用 PostgreSQL 数据库的可选持久化检查未执行。
- 13 组管理 HTTP 契约通过，覆盖标准 Fleet 响应、旧 Hafleet 路径/输入兼容、
  所有者隔离、标识冲突拒绝及完全一致的配对凭据。
- 原生接入契约通过，覆盖新旧配置键、数量上限、幂等凭据及所有者隔离。
- 客户端 PKCE/账号绑定/token 撤销回归、两端 Rust Clippy 和两端前端构建通过。
  服务器前端原有的未使用代码警告仍存在。
- 真实 Rust 客户端/服务器与受控 Pasion/Matrix 的接入集成通过：自动导入、
  outbound poll、App Service 投递、精确 probe 回执与 reception 验证。
- 受控浏览器 fixture 确认 `/hagency/fleets` 显示 **My Fleets**，侧栏显示
  **Fleet connections**，并保留 Matrix/Padmin 导航。
- 既有 ID、注册、token、协议字段与审计历史保留，无需数据库迁移。
