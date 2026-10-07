# Owner Agent v1：Docker 发布验证

Docker 镜像从固定的 Cargo lock、Palpo/Pasion 源码 revision 和本仓库源码构建；它内置必要的 Agent appservice。服务以 UID 10001 运行，私密配置和持久身份文件保存在 0700 目录、0600 文件中。`/healthz` 只证明 HTTP listener 可用；镜像 HEALTHCHECK 使用 `/readyz`，要求真实 Matrix → appservice 启动探针往返成功。

## 隔离验收

在仓库根目录先构建新的镜像，再运行测试：

```sh
docker build --progress=plain -t hagency-server:owner-appservice-v1 .
HAGENCY_TEST_IMAGE=hagency-server:owner-appservice-v1 node tests/docker-smoke.mjs
```

测试拒绝缺少 `org.hagency.protocol=owner-agent-v1` 标记的旧镜像。需要核对具体构建快照时，构建时传入 `--build-arg HAGENCY_SOURCE_FINGERPRINT=<sha256>`，测试时传入同一值 `HAGENCY_TEST_IMAGE_SOURCE=<sha256>`；测试会比对镜像 label。这是源码快照标识，并非容器镜像签名。

测试自动生成独立 Compose project、随机 localhost 端口、私密临时配置和两个新的 named volumes。数据库实例内包含 Hagency、Palpo、Pasion 三个数据库；退出时仅删除这次测试的 project/volumes 和临时目录，不操作开发项目或既有数据卷。PostgreSQL 和 Docker Engine 必须可用。

验收覆盖：

- 新数据库 schema、唯一强制 AS 注册、持久启动往返回执、镜像 healthy 状态；
- Pasion 管理员登录、非管理员注册及原生 public client 的 DCR/PKCE/consent、Matrix 身份对应；
- 普通成员通过个人 Matrix OAuth 创建 Space 和 Room，随后 adopt 并创建 Agent，异步等待傀儡加入 Room；
- Project/Room roster、不可变 Agent owner、精确 binding 的事件 poll、先 ACK 后 start、相同 execution 的幂等 start；
- 设备租约续期、已知回复 durable outbox、实际 Matrix 傀儡消息及 thread 关联；
- 前端模块/WASM 和 Pasion 页面/issuer/JWKS 可访问，旧 Fleet 等 API 返回 404；
- 非 root 进程与私密文件权限；
- 移除初始 bootstrap 参数后重启，AS/Matrix/Pasion 密钥及 Agent/设备身份持久保留，旧内存浏览器 session 不复活；
- 重启后重新登录、获取新设备 generation 并再次接收、回复实际 Matrix 事件。

测试提供固定的已知回复文本，**不启动 Codex、不调用付费模型、不开放原生工具**。HTTP/WASM 资源检查不等于浏览器视觉验收。Matrix/Room 权限撤销、备份恢复、额度和本地工具授权仍由各自专项验证覆盖。

## 首轮验证记录（预算连续性修复前）

2026-10-07 在本机 Docker Engine 29.8.0、Compose 5.5.1、aarch64 的新镜像实际执行通过：

- 镜像：`hagency-server:owner-appservice-v1`
- 镜像 ID：`sha256:a56675858e7363d1260bea1c88c575f31f6c43f76beb149b626b6ec7d8bdf167`
- 构建输入 SHA256：`773e2be5b922015f8a163adf591d31a45df0bcebad68b91ccf88fec0d5ae8b74`
- 最终 `docker build` exit 0；完整隔离 `tests/docker-smoke.mjs` exit 0；验证后构建输入指纹仍一致。
- `cargo test -p hagency-xtask --offline`：1 个 unit 和 4 个 integration 全部通过；配置初始化拒绝覆盖、私密权限及资源构建检查通过。
- `node --check tests/docker-smoke.mjs`、`sh -n scripts/docker-entrypoint.sh`、`docker compose config --quiet`、`git diff --check` 均通过。
- 测试 project `hagency-owner-test-1d186354cf99` 的容器、卷、网络已全部清理；既有 `hagency-server-postgres-1` 始终未被操作，清理后仍 healthy。

构建输入指纹按顺序包含 `Dockerfile`、`Cargo.toml`、`Cargo.lock`、`scripts/docker-entrypoint.sh`，然后包含 `crates` 和 `xtask` 内排序后的全部文件（排除路径组件 `target`）；对每项加入相对路径、NUL、文件内容、NUL，再计算 SHA256。仓库后续变化必须重新构建验收，不能沿用此记录证明新源码。


## 预算连续性修复后验证

2026-10-07 针对最终冻结源码重新构建并重新执行完整隔离测试：

- 最新镜像 ID：`sha256:ef05f8c5e3d473cf96c93f3ec647a1e4bb33330c59993be79e154e3a05d8fe3b`
- 最新构建输入 SHA256：`603069ed7b2ea2e6ae403fb02721f57fc605f7533de5d800106ca6c1fe823acb`，包含 158 个输入文件；验收后重算仍一致。
- `docker build` exit 0；使用这个指纹的完整 `tests/docker-smoke.mjs` exit 0。
- 全新部署和重启后均读取 `POST /execution/history` 的完整分页结果并重算快照 SHA256。租约获取必须提供这个快照；错误快照返回 409 `execution_history_changed`。
- execution 的幂等 start 只产生一个永久历史见证。start 后用旧快照尝试接管返回同一 409，原租约仍可以续期；拒绝路径没有撤销当前执行权。
- 重启后历史 count、execution ID 和 immutable digest 保持一致，完整普通成员创建/收发/重启链路再次通过。
- 测试 project `hagency-owner-test-3a895e776d62` 的容器、卷和网络全部清理；原开发 PostgreSQL 仍 healthy，未被操作。

这些 Docker 验证证明服务端历史见证、原子快照接口和真实部署链路，不把服务端说成模型费用审批者。客户端独立验证完整本地费用证明，缺失时返回 `ledger_recovery_required` 或进入 `reconciliation_only`，阻止新模型工作，仅按可证明的既有结果恢复回复。额度恢复界面与本地费用 gate 由客户端专项测试证明；Docker 固定文本回复不会推理或计费。
