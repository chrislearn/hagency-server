# 新 owner 架构的备份与恢复

本手册仅适用于全新 Hagency 数据结构；不导入旧 Fleet 数据，也不提供 Agent 转让或改 owner 的恢复办法。

## 一致性边界

服务端备份包括独立 Hagency PostgreSQL 数据库、部署配置和私有 Appservice 注册凭据。永久 owner、傀儡 MXID、issuer/subject/MXID、命令去重摘要、binding generation、租约 epoch、收件事务摘要和 outbox 的稳定 Matrix transaction ID 必须一起保留。只复制 Agent 表不能恢复可靠运行。

Palpo 与 Pasion 分别使用自己的备份机制，并记录三者恢复点。Hagency 恢复不初始化或清空其数据库。若 Palpo 恢复点丢失了 Hagency 已记录的 Matrix 事件，不能把本地 `sent` 凭证改为未执行并重新调用模型；先在隔离环境核对事件、原 transaction ID 与原回复。跨系统恢复点不一致是人工核对事项。

客户端备份由创建者在停止 host 和用户服务之后执行，包含整个私密 owner 状态目录及账本、Room 文件工作区和 Codex 上下文。只备份 SQLite 主文件而忽略 WAL 不安全；停止进程后再复制整个目录，或使用 SQLite 正式备份 API。系统 keyring 的提供方登录不在目录备份中，恢复后由原用户重新登录。Pasion 会话也需重新登录，不把遗留 bearer 当作新授权。

## 服务端演练顺序

1. 记录服务器部署 identity、固定 Pasion issuer、homeserver、代码版本和新 schema 版本；暂停 Agent 服务并停止 Hagency 后台 worker，防止备份期间产生新执行或发送。
2. 使用 PostgreSQL 的 `pg_dump --format=custom` 备份完整独立 Hagency 数据库；密码由正常数据库凭证机制提供，不写进 shell 参数、日志或报告。私有 AS 注册文件分别加密备份，保留权限。
3. 将备份恢复到全新隔离数据库及相同身份配置的隔离实例。不要恢复覆盖生产或将备份里的 AS 注册安装到不同的真实 homeserver。不要同时启动两个共享同一部署身份的对外实例。
4. 核对 owner/Agent/MXID、binding/generation、原 dispatch/execution、原回复 digest/transaction ID、AS transaction digest。schema 不匹配时停止，不手改版本或触发兼容迁移。
5. 原设备会话与租约视为不可信，撤销旧设备并显式 takeover，或等待旧租约实际到期；撤销设备本身不缩短已记录的旧租约 TTL。原 owner 重新通过 Pasion 登录并取得新设备授权。后台状态将失去执行权的 running 封存为 unknown；不能将 unknown 改回队列或清除成本保留。
6. 已知持久回复通过 `execution/replies/reconcile-known`，在新的有效设备租约下提交原 dispatch/execution、相同内容及相同 transaction ID。已发送记录保留 event ID；未知发送复用原 transaction。不可逆投递禁止状态不能借恢复清除。
7. 逐 scope 核对当前 Matrix Space/Room 关联、双方成员和暂停状态，再由 owner 明确恢复绑定。测试启动探测事件往返，并分别检查设备在线、模型登录和任务结果；`/readyz` 只代表本次启动曾完成 AS 往返。

已完成同机隔离环境下三个完整数据库的离线 dump/restore、原部署身份重启、重新登录与新设备接管，以及 unknown 不重执行、已发送回复身份保留和新请求往返验证，见[实际演练记录](BACKUP_RESTORE_VALIDATION.zh-CN.md)。追加独立文件系统恢复也已通过：原目录移走使旧路径不可达，从备份恢复新私有目录及三库，AS/Matrix/Pasion 密钥、配置 pepper 保持原字节，真实 Matrix 媒体重新下载一致。整机/异机、外部对象存储、Pasion 媒体 API 和发布环境灾难恢复仍需独立验收。

## 数据保留与容量

AS 原始明文事务仅在完整路由完成后压缩；事务 ID 与规范化内容 digest 永久保留以识别相同重试或冲突。待路由事务继续占有界活跃队列。启动探测事件凭证单独短期保留。永久去重元数据仍持续增长，数据库容量需要监控，不能宣称总历史无界且无成本。

本地 unknown 用量、未知模型执行、未确认 Matrix 送达的回复和工具副作用凭证不得按普通日志轮转删除。工具 receipt 与额度账本是执行证据。备份或恢复也不能消除这些保留，或借同名 Agent 重建改变其终身主人。
