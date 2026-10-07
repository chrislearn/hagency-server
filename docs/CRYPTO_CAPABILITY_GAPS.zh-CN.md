# 新 Appservice / 永久 owner 架构的 E2EE 能力核对

核对日期：2026-10-07。结论：保留并复用客户端 SDK 加密实现是合理的；当前新架构尚未实现可上线的受限傀儡加密通道，继续明确拒绝 encrypted Room，不降级发送明文。无需为了做第一阶段 E2EE 修改 Palpo 默认功能，但不能将普通 puppet access token 发给客户端后宣称它具有 Room 限制或设备撤销联动。

## 证据边界

本报告读取 server Cargo.lock 实际固定的 Palpo `c8568d9844a6be0a3172d98d7c9810e3a1f7521c`：`~/.cargo/git/checkouts/palpo-822f758f73f2b85d/c8568d9/`。另一个本地 Palpo 工作目录 HEAD 不同，不能拿其行为代替运行依赖。源码能力不等于互操作已验收；本轮没有修改 Palpo，也没有运行真实加密 Room 端到端联调。离线 SDK proof 只证明本地设备密钥、重启和加解密，不证明新 server proxy、撤销、Room 授权或在线密钥交换。

| 能力 | 固定版本实际实现 | 新 Hagency 缺口 |
| --- | --- | --- |
| 傀儡设备登录 | `routing/client/session.rs` 的 `m.login.application_service` 验证 AS token 和 namespace、要求用户存在，创建/更新确切 user/device 并签发设备 token；返回 `expires_in: None` | 安全保管 token；持久映射永久 owner、Agent、Hagency installation/device generation、Matrix device、crypto generation。禁止 client 任意选择 puppet/device |
| AS device masquerade | `hoops/auth.rs` 使用 AS token + `user_id`/`device_id` 查 namespace 并取得/创建设备 | AS token 只能留在服务端；固定推导身份的内部适配器可用，但普通 DTO 不能提供此类身份参数 |
| 设备密钥/OTK | `routing/client/key.rs` 提供 upload/query/claim；upload 用认证 user/device 写入 | 签名/public key 绑定该映射；query/claim 的用户/设备集合须限定活跃授权 Room 的真实成员，不能开放任意 Matrix 代理 |
| fallback keys | upload 明确 `TODO: fallback keys`；`sync_v3.rs` 返回 `device_unused_fallback_key_types: None` | 不能宣称支持完整 fallback 协议；第一阶段采用足量 OTK、补充/耗尽恢复测试，不改 Palpo、不伪造成功响应 |
| 同步及设备通知 | `routing/client/sync_v3.rs` 用认证 user/device；`sync_v3.rs` 返回 Room、to-device、device_lists、OTK counts | 新服务没有受限 crypto sync；SDK 需要完整可用 crypto 元数据，不能只转发 AS timeline transaction |
| to-device 发送 | `routing/client/to_device.rs` 提供 sendToDevice、user/device/txn 去重，可向任意目标及 `*` 发送 | 封闭 event type、具体 recipient/device、payload/频率/容量；禁止任意目标和通配 `*`。Olm ciphertext 内部的 Room/消息类型不可被不持钥服务器证明 |
| cross-signing | `key/device_signing.rs` 初次无 master keys 时可不走 UIAA；已存在密钥的本地/AS 会话仍有 UIAA 路径；不能当成 delegated human OAuth | 不可把 owner 人类 token 冒充 puppet，也不可自动重置既有 cross-signing。第一阶段明确新设备信任/验证合同，恢复/更换主身份另列验收 |
| 撤销/退出 | `session.rs`/`device.rs` 已有标准 logout、logout/all、设备删除，删除设备操作涉及 UIAA | Hagency device 撤销不会自动撤销 Matrix puppet token。proxy 每次重验可以拒绝后续授权，映射 token 的注销/吊销需单独可重试任务；不能暴露 logout/all 删除其他合法设备 |

## 客户端确实已有的部分

`native/hagency-matrix/src/sdk.rs` 使用 Matrix SDK 与 SQLite crypto/state store；`sdk/keys.rs`、`sdk/encrypted_message.rs`、`sdk/outgoing.rs`、`sdk/enrollment.rs` 含设备信任、Olm session、Megolm 分享/加密、签名和送达日志；`sdk/approval_intake.rs` 等已有 sync ingest。`native/hagency-crypto-proof/src/lib.rs` 有 encrypted SQLite store 重开、错误 device/store key 拒绝、加密后重启解密 proof。client Cargo.toml 固定 `matrix-sdk-crypto = 0.18.0`。

这些是可复用组件，不是新 server 的权限实现：旧 enrollment/approval/代表身份和直接 Matrix transport 需要改成永久 owner + server device + lease 的新合同。crypto store 的 identity fence、私密权限、持久 cursor、未知写入恢复不能在适配时绕掉。不要迁移旧 Fleet 数据或恢复旧 native enrollment。

## 受限 crypto proxy 可行的最小合同

1. 每个 Agent/owner installation 建立独立、持久 Matrix crypto device 映射，不按短 lease epoch 每次新建；lease 控制运行权，crypto generation 控制密钥设备身份。注册由当前已认证 owner device 请求，服务端从 Agent 永久 owner 推导 puppet 和 device。Matrix token/AS secret 都不回传 client，私钥及 crypto store 只留在 owner 本地。多个 owner devices 的共享/迁移不默认实现；接管必须明确新 crypto device、原身份未知写入与重新分享策略，不复用别人私钥。
2. 新封闭 crypto API 每项在同一可信授权边界重验 session expiry、device revocation/generation、永久 owner、Agent state、binding generation、管理员/owner pause、实际 owner Space/Room membership 与 lease/crypto generation。不能开放任意 path/method/query/actor/device/recipient。设备撤销后拒绝新 sync/key query/key claim/key upload/to-device/send 授权；已经送出的网络请求和已经持有的明文/密钥不能撤回。
3. 服务端持有真正 Matrix device token 或仅内部使用 AS masquerade。后者没有 raw token 自动过期带来的安全边界，所以也必须靠相同 proxy 校验。对客户端完全不发行可绕 proxy 的 Matrix bearer；仅靠隐藏 homeserver URL、客户端承诺 Room filter 或 bearer TTL 不满足限制。
4. 先支持 keys/upload、keys/query、keys/claim、受限 sendToDevice、传统 `/sync` 的 SDK 所需子集。拒绝 arbitrary account_data、备份/secret storage、dehydrated device、管理员 API、任意 Room history/media、任意设备管理；若 SDK 请求未支持操作明确失败，不能 silently ACK。keys/signatures 与 device_signing 要独立校验签名 owner/target/初始化状态，非第一阶段必需项可以明确关闭。
5. 接收端以 Agent 当前所有授权 active bindings 的 Room 集合作服务端裁剪，过滤 timeline/state/invites/presence/account_data；设备列表和 query/claim 集合来自真实成员，不由 caller 提交全量名单。to-device 是设备级队列，不能仅按 timeline Room filter 过滤；加密 payload 无法证明属于哪个 Room。可按已授权成员+确切 target device 限制外层并由本地 SDK 拒绝非授权 Room key，但不能声称服务器已验证密文内 Room 归属。若要求严格逐 Room 密钥隔离，需要额外的 per-binding crypto device 设计与发送方互操作测试；同一 puppet 的多 device 仍需客户端拒绝误发/跨 scope keys，不能保证远端永不误送。
6. 实现 durable sync mailbox：上游 Palpo 在下次 `since` 时删除上次 to-device 事件，故必须先持久保存响应/cursor，再允许上游确认，客户端本地 crypto journal 持久处理后才确认下游。固定一条上游同步流水线，lease 接管/重启不得并行拉同一 device/cursor；response、to-device 去重、cursor、大小上限、过期后返回前再校验需要专门测试，避免丢失 Room key。
7. Encrypted timeline 的 mention/body 位于密文，现 server worker 不能判断触发条件。不能沿用“只 plaintext mention/thread 路由”而宣称解密完成。加密事件按授权 binding 持久投递 ciphertext，由本地 SDK 解密并判断 mention/thread/本地策略；canonical event ID、sender、room 来自可信 Matrix，不接受 client 伪造 requester/room。多 agent 同 Room 的密文扇出和本地配额成本需明确。server reply outbox 存稳定密文及 txn ID，仍走现有 owner/generation/lease/fresh facts 授权；不得先发明文回退，也不得重试时重新随机加密同一 txn。

## 最小实施阶段与验收门槛

- A：独立 crypto device 映射/schema、服务端凭据密封保存、封闭 API 和撤销/generation/lease 校验；初始化/注销的 durable reconciliation，失败重试不重建错 device。单元/PG 证明所有伪 owner、过期/撤销 device、跨 binding、旧 generation、任意 recipient/Matrix path 都拒绝。
- B：SDK transport 接入受限 proxy、crypto store 绑定新身份、durable sync/to-device cursor/ACK、OTK upload/query/claim 与设备信任；先一 owner installation、单 Agent，后多 Room。真实 pinned Palpo 双设备 Olm/Megolm 联调，模拟 cursor 回放、重启、钥匙延迟、OTK 耗尽、断网与设备撤销。
- C：ciphertext queue 和本地触发判定、加密 durable reply、稳定 txn/ciphertext 重试、接管不重跑旧 execution；撤销 binding 后禁止新密文投递/发送，即使客户端已有旧密钥也不能通过 proxy 继续访问。验证人类 Matrix 客户端能收发，SDK trust 判定没有被手工强制放行。
- D：必要时单独实现 cross-signing 恢复/verification/key backup 与严格 per-binding device 隔离。明确 deferred，不阻塞第一阶段 basic E2EE，但不能对外宣称这些已实现。

这至少包含三个独立工程面（凭据/授权、可靠 crypto 同步、加密业务队列）及真实互操作测试，不是删除 `encrypted` 拒绝条件或转发几个 keys endpoint。现有 SDK 大幅减少密码学实现量，但没有消除新架构的授权和恢复工作。在 A–C 通过前继续延期并明确 UI 状态；不修改 Palpo 默认功能，不增加资源审批，不支持 Agent 转让。

## 本轮记录的验证

在 client 运行 `cargo test -p hagency-crypto-proof --offline`，
`crypto_device_survives_restart` 1/1 通过。仅验证加密 store 重开、身份绑定和本地加解密；不代表真实 Palpo 加密 API 或拟议 proxy 已验收。
