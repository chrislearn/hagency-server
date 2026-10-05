# Hagency Operations

[中文](README.zh-CN.md) · [Architecture and migration](../../docs/OPERATIONS.md)

`hagency-operations` owns Hagency business workflows. It is a Rust/Salvo library
mounted by `crates/backend`, not another server to deploy. It depends on
`hagency-contract`, PostgreSQL/Diesel and Matrix HTTP APIs; it has no dependency
on Palpo crates, Node.js or SQLite.

It provides native Matrix-authenticated sessions, project/agent/token top-up
Inbox decisions, role-scoped reads, immutable commands, durable receipts,
notification intents and a Matrix notification worker. The host shares its
existing PostgreSQL document and fleet delivery queue with these workflows.

The canonical native endpoint is `/_hagency/miniapp/v1/`. The former
`/_palpo/miniapp/v1/` endpoint and `palpo.*` service names remain compatibility
aliases. The web frontend uses the host's cookie/CSRF adapter at
`POST /api/operations/call`, and displays Inbox at `/hagency/inbox`.

Approving a request does not prove that it has executed or that its agent can
chat. Runtime receipts, current observations and capacity reservation remain
separate checks. New authority is imported by an explicit offline operator
command; this import is not automatic proof of a live owner delegation.

Run `just check-operations`. For PostgreSQL restart tests, use an **empty,
dedicated** database and run:

```sh
HAGENCY_TEST_DATABASE_URL=postgres://... cargo test --locked -p hagency-operations \
  postgres_shared_writer -- --ignored
```

Current boundaries and remaining runtime/client acceptance work are documented
in [English](../../docs/OPERATIONS.md) and [中文](../../docs/OPERATIONS.zh-CN.md).
