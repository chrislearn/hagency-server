# 隔离三库与独立文件系统恢复演练证据

[English](BACKUP_RESTORE_VALIDATION.md) · [恢复合同](RECOVERY.zh-CN.md)

2026-10-07，真实嵌入 Hagency/Palpo/Pasion 后端通过 `scripts/test-agent-integration.py` 的可选恢复演练。在服务端仓库执行：

```sh
HAGENCY_TEST_BACKUP_RESTORE=1 python3 scripts/test-agent-integration.py
```

此开关自动启用原生 Pasion DCR/PKCE/授权流程。最终独立文件系统恢复仅设置此开关，完整运行通过，没有调用模型或工具执行器。脚本创建三个随机 fixture 库，在全部备份前停止唯一服务实例，通过 `pg_dump --format=custom` 与 `pg_restore --exit-on-error` 完整恢复到另外三个新随机库。完整备份 fixture 的 data/media/config 并恢复到另一个全新私有临时目录，仅替换数据库地址及本地文件根路径，保持 issuer、homeserver、端口、AS namespace/注册身份，随后启动唯一实例。重启前将原目录移为 offline，使所有原路径不可达；恢复进程使用自己的日志文件，不能悄悄读取原身份或媒体文件。不覆盖源库，不同时运行两个相同 AS 身份实例。备份和配置权限私有，不输出秘密或备份内容。源库与恢复库都在 `finally` 删除，成功的原/offline 与恢复私有目录都已删除。

重启前逐一核对：永久 Agent 主人和傀儡账号的完整记录、全部命令记录及请求摘要、全部 AS transaction ID/摘要、原先已发送回复的完整 outbox 记录完全一致。重启后保留所有旧 AS 摘要，真实新 startup AS 事件通过就绪门禁，AS 注册文件、Matrix 签名密钥、Pasion 签名/加密 secrets、配置的密码 pepper 在重启后逐字节不变。调整配置位置前，所有复制文件均核对 SHA-256 与权限；必需的私有密钥/config 保持私有权限，不输出摘要或秘密字节。

原主人使用恢复后的密码 pepper 重新获得 Pasion 原生授权并创建新设备。停机前真实上传的 Matrix 媒体，在原存储路径不可达时，经恢复后的认证媒体 API 下载，字节完全一致。撤销旧设备后旧设备 poll 返回 401。撤销不会立即缩短数据库中的旧租约 TTL，因此恢复过程显式执行主人接管，推进 epoch 并封锁旧执行。快照中原先 running 的模拟执行变为 `unknown`，新 epoch 无法再次 start，poll 不再将其返回为待执行任务；没有清除未知执行或费用/副作用的不确定证据。

已发送的已知回复通过显式 reconcile 返回相同 Matrix transaction、正文及 event ID；恢复后读取真实 Matrix 原事件，傀儡 sender 与正文不变，原回复整行保持不变。新 mention 在新设备下完成 poll、ACK、start、持久回复和真实 Matrix 投递。后续退役及真实迟到入 Room 也收敛成功，没有重新模型推理。

## 按实际源码核对的持久路径

| 内容 | fixture 路径 | 来源 |
| --- | --- | --- |
| AS/HS 凭据与注册 | `data/agent-appservice.json` | 必装 Appservice 初始化 |
| Matrix 签名密钥与版本 | `data/matrix-signing-key.json` | backend `Config::prepare_signing_key` |
| Pasion 签名/加密密钥及 Matrix shared secret | `data/pasion-secrets.json` | backend `pasion::prepare` 持久生成的 `secrets` 与 `matrix_secret` |
| 密码哈希 pepper | `password-pepper` | fixture `[passwords.schemes]` v1 Argon2id `secret_file`，按 Pasion config 相对路径解析 |
| Matrix 本地媒体 | `media/` | fixture Palpo `[storage]` root，真实上传/下载验证 |
| Pasion 本地媒体 | `data/pasion-media/` | backend 管理的 storage root，完整 data 树复制，未声称经 Pasion 媒体 API 上传验证 |
| Host/Palpo/Pasion 配置 | 根目录三个 TOML | 运行身份不变，仅调整数据库/本地文件位置 |

Pasion 默认密码方案没有 pepper。本次在账号 bootstrap 前显式配置私有 pepper 文件，恢复后密码登录成功才能证明此依赖已恢复，不声称默认自动生成额外 pepper。仓库 binary/static resources 和 PostgreSQL 角色/扩展是现存测试依赖，不属于复制的私有 fixture 资产。

## 实际边界

本次是同一主机和 PostgreSQL 集群上的一致停机三库及独立文件系统/密钥/媒体恢复；恢复时原路径不可达。它证明私有文件依赖复制、完整关系数据恢复和服务重启，不等同于生产整机灾备演练。PostgreSQL 角色/扩展、操作系统/runtime 安装、外部 object storage、主人本地 keyring/crypto 状态、部署特有的外部 secret 文件仍须另外处理。未覆盖整机丢失、跨版本迁移、在线跨库快照/PITR、联邦持续性、加密 Room、快照过程的并发发送故障。生产演练须按部署依赖补齐，不改变 homeserver/issuer、不运行重复 AS。本次验证已知 sent 回复；网络投递不确定的回复仍须按稳定 transaction 恢复，不能重新推理。
