# Hagency coordinator workflow contract

[中文](README.zh-CN.md)

First Rust implementation slice of [Rinx ADR 0011](https://github.com/hagency-org/Rinx/blob/main/docs/adr/0011-hagency-server-engagements.md).

The Matrix administrator authorizes server associations. The resource owner's
assigned Hagency coordinator approves projects and agents. An authorized owner
may also approve agent allocations. One Rinx mini-app approval is sufficient;
Hagency automatically checks authority, reserves capacity and provisions it.

This crate implements versioned command data, distinct server-engagement and
agent-allocation IDs, current-state policy checks and checked budget arithmetic.
It is owned by hagency-server and has no Palpo crate dependency. The mounted
`hagency-operations` library supplies endpoints and persistence. This contract
alone does not authenticate callers or execute an allocation.
Do not advertise `hagency.coordinator_approval.v1` merely because this crate builds.

## Trust and transaction boundaries

- Authenticate Matrix users in the server adapter. At Hagency, authenticate the
  machine envelope and verify the actor proof; never trust a body-supplied actor.
- Load engagement/coordinator, request and project state from trusted storage.
  Deserializing these types does not authenticate their source or delegation.
- Call the policy function in the transaction that checks pending state/revision,
  records the verdict and inserts the outbox command. Its `Ok` alone is not a
  persisted approval. Validate that requested resources are funded, visible and
  belong to the engagement; duplicate resource IDs are refused by the contract.
- Canonicalize the full immutable agent definition, including resource settings,
  target rooms and limits, identically on both sides before enabling the protocol.
- For execution, recheck registration/delegation/project/request revisions and
  expiry, then reserve the engagement's remaining capacity and record the command
  receipt atomically. Serialize competing approvals in the database, not here.
- A replay with the same command ID and identical content returns the stored
  receipt; a changed payload conflicts. Fetching a prior result never executes
  revoked work again. Crash-safe outbox and receipt storage are integration work.
- The first approval can grant a positive amount no greater than requested.
  An increase is a separate top-up request against the existing agent allocation.
  `TokenTopUpApproval` freezes the existing agent, expected allocation and
  requested addition. The same coordinator/owner policy applies; adapters must
  compare the actual allocation and remaining parent capacity atomically.

The shared canonical encoder matches JavaScript UTF-16 object ordering,
array-index keys and finite transport numbers. Signed operation definitions
permit only exact JSON integers; transport events additionally allow finite
floating-point values.

`BudgetSnapshot` describes one explicit account/resource/period scope selected by
the trusted writer. Available = allocated - consumed - reserved unused. It does
not release spent tokens or supply synchronization. Missing allocations and
overdrawn observations refuse new capacity. Amounts stay within exact JSON integer
range for OctoScript consumers. An engagement allocation is reserved at its parent;
allocating an agent within it must not charge the same parent reservation twice.

## Integration

The server mounts [Hagency Operations](../operations/README.md) and persists
verdicts, outbox records and notification intents in the existing Hagency
PostgreSQL database. Runtime adapters must import this crate from
`chrislearn/hagency-server` and recheck its policy within their actual capacity
reservation transaction. Do not move resource ownership or runtime quota
accounting into a Matrix homeserver.

The crate uses standard Matrix identifier validation from Ruma, not Palpo
internals. Its command schema and canonical digests retain the upstream wire
version, so moving ownership does not rename serialized IDs or alter signatures.
The old agent-allocation engagement ID remains distinct from a server engagement.

See [architecture and migration](../../docs/OPERATIONS.md) for current boundaries,
legacy-state preservation and outstanding live acceptance work.

Run `cargo test --locked -p hagency-contract`.
