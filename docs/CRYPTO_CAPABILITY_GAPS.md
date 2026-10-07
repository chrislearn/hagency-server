# E2EE capability gaps in the permanent-owner Appservice architecture

Reviewed 2026-10-07. [中文版本](CRYPTO_CAPABILITY_GAPS.zh-CN.md).

The existing client crypto implementation is substantial and should be retained.
The new owner/device/lease server architecture does not yet have an operational
restricted crypto channel. Continue rejecting encrypted Rooms explicitly; do not
send plaintext as a fallback. Basic E2EE appears feasible without changing Palpo
itself, provided a bounded server proxy and recovery protocol are implemented.
A puppet access token handed to the client does not meet the required boundary.

## Actual source reviewed

The authoritative Palpo revision is the server Cargo.lock pin
`c8568d9844a6be0a3172d98d7c9810e3a1f7521c`, read from Cargo's git checkout. The
separate local Palpo work directory has a different HEAD and is not evidence for
the running dependency. This is a source capability audit, not a live encrypted
Room interoperability pass. No Palpo source was modified.

| Surface in pinned Palpo | Existing capability | Required Hagency boundary |
| --- | --- | --- |
| `routing/client/session.rs` | AS login validates AS token/namespace, checks user and creates/updates a concrete device/token; login response has `expires_in: None` | Keep AS and Matrix credentials server-side; derive puppet/device from immutable owner and registered installation, never caller-supplied identity |
| `hoops/auth.rs` | AS token supports namespace-restricted user/device masquerade | Internal adapter only; not a public token, free-form query or general proxy |
| `routing/client/key.rs` | Device-authenticated upload, key query and one-time-key claim | Enforce mapped device and authorized recipients from current Room membership; validate identity/signature fields and limits |
| `key.rs`, `sync_v3.rs` | Fallback key upload is explicitly TODO; sync reports no unused fallback key types | Do not claim fallback support or manufacture success; test OTK replenishment/exhaustion |
| `routing/client/sync_v3.rs`, `sync_v3.rs` | Device sync includes Rooms, device lists, OTK counts and to-device events | Durable upstream/downstream cursor and delivery acknowledgement, scoped response, correct SDK crypto metadata |
| `routing/client/to_device.rs` | Transaction deduplication; arbitrary recipient/device and wildcard support | Fixed types, concrete recipients/devices, bounded payloads; no wildcard or arbitrary messaging tunnel |
| `key/device_signing.rs` | Initial cross-signing upload may avoid UIAA; replacing existing keys in local/AS sessions follows UIAA rules | Explicit initialization/trust/recovery contract; never substitute a human OAuth token for puppet identity or automatically reset signing keys |
| `session.rs`, `device.rs` | Logout and device management exist; deleting devices involves UIAA | Hagency revoke does not automatically revoke puppet credentials; separate durable credential cleanup, no public logout/all over other devices |

Client source worth reusing: `native/hagency-matrix/src/sdk.rs`, its `sdk/keys.rs`,
`encrypted_message.rs`, `outgoing.rs`, `enrollment.rs` and sync ingest code, plus
`native/hagency-crypto-proof/src/lib.rs`. They provide SDK-backed crypto/state
stores, identity fences, device trust, Olm/Megolm, signatures and durable outgoing
work. `matrix-sdk-crypto` is pinned to 0.18.0. Legacy enrollment/approval identity
and direct Matrix transports are not the new authorization model; adapt those
boundaries without importing Fleet data or resurrecting enrollment APIs.

## Smallest viable restricted proxy

1. Persist a Matrix crypto device per Agent/owner installation, distinct from a
   short execution lease epoch. Bind it to owner identity, Hagency device and its
   generation, Agent and crypto generation. Keep private crypto state exclusively
   local; keep raw Matrix/AS bearers exclusively on the server. Takeover cannot
   assume private keys can be copied or an old ambiguous write can be rerun.
2. For each closed crypto operation revalidate live session/device generation,
   revocation, permanent owner, Agent/binding state, pause, lease and real
   Space/Room membership. No arbitrary endpoint, identity, recipient, query or
   HTTP method. Reject new authorization after revoke; already issued network
   requests, delivered plaintext and cached keys cannot be recalled.
3. Implement only required upload/query/claim, bounded sendToDevice and a classic
   sync subset first. Deny unsupported requests explicitly rather than silently
   acknowledging SDK work. Arbitrary account data, backup/secret storage,
   dehydrated devices, admin/device management, media/history and wildcard sends
   remain closed until separately specified. Cross-signing/signatures need their
   own exact target and initialization validation.
4. Filter Room timelines/state/invites and metadata against current active
   bindings. Key query/claim/device lists use real authorized members. To-device
   is a device queue: an Olm encrypted payload hides its internal Room ID and
   event type from the keyless server. Outer sender/recipient restrictions plus
   local rejection of unauthorized Room keys are possible; server-proven exact
   key-to-Room isolation is not. Strict per-binding device isolation requires a
   further device and interoperability design and cannot prevent a remote sender
   from accidentally addressing a key to the wrong device.
5. Build a durable crypto sync mailbox. Palpo removes previously received
   to-device events when a subsequent sync acknowledges its `since` cursor.
   Persist an upstream response before advancing upstream, and wait for the
   client's durable crypto journal before downstream acknowledgement. One ordered
   upstream stream per Matrix device, bounded responses, replay/deduplication and
   post-wait authorization rechecks are necessary to avoid losing Room keys.
6. Encrypted bodies hide mentions. The current plaintext mention/thread worker
   cannot trigger correctly on ciphertext. Route authorized encrypted events to
   the local SDK, which decrypts and applies mention/thread/local policy. Canonical
   event ID, sender and Room remain trusted Matrix fields, not client claims.
   Multiple Agents in a Room imply ciphertext fanout and local processing costs.
   Store a fixed ciphertext and transaction ID in the reply outbox; retrying must
   not re-encrypt to a different payload under the same transaction. Existing
   owner/generation/lease/fresh membership fencing remains mandatory.

## Implementation and acceptance phases

- **A — credential/authorization:** device mappings, sealed server credentials,
  closed APIs and durable init/revoke cleanup. PG/contract tests reject wrong
  owner, revoked/expired devices, old generations/leases, cross-binding access,
  arbitrary recipients and Matrix paths.
- **B — reliable crypto transport:** adapt SDK transports/stores to these identities,
  durable sync/to-device cursor/ACK, OTK exchange and explicit trust policy. Prove
  real Olm/Megolm with pinned Palpo, delayed keys, restart, cursor replay, exhausted
  OTK, network failure and revocation. Start with one owner installation/Agent;
  expand to multiple Rooms after isolation tests.
- **C — encrypted Agent traffic:** ciphertext queue/local trigger decisions,
  encrypted durable reply, stable transaction recovery and takeover without
  reexecuting old unknown work. After binding revoke, the proxy rejects further
  delivery/send even when local old keys still exist. Verify a real human Matrix
  client interoperates without forcibly marking devices trusted.
- **D — optional follow-up:** cross-signing replacement/recovery, verification,
  backup and strict per-binding crypto devices. Mark these deferred explicitly.

The minimum is three engineering areas—credentials/authorization, reliable crypto
sync and encrypted business delivery—plus live interoperability tests. Existing
SDK code saves implementing cryptography, but deleting an encrypted-room guard or
forwarding a few key endpoints would not finish this work. Defer release until
A–C pass; preserve Palpo defaults, local resource autonomy and permanent ownership.

## Verification recorded

`cargo test -p hagency-crypto-proof --offline` in the client passed its one
`crypto_device_survives_restart` test on this review run. This validates encrypted
store reopen, identity binding and local encrypt/decrypt only; it does not validate
live Palpo crypto APIs or the proposed proxy.
