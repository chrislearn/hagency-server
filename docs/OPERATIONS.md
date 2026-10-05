# Hagency business ownership and migration

[中文](OPERATIONS.zh-CN.md) · [Documentation](README.md)

## Component boundaries

| Component | Owns | Does not own |
| --- | --- | --- |
| Palpo / MatrixServer | Matrix client/federation protocols, identity verification, rooms, events, App Services and Matrix administration APIs | Hagency projects, resources, coordinators, budgets, Inbox or agent approval |
| Pasion | Accounts, OIDC/OAuth, sessions, Matrix token authentication and account roles | Resource delegation or quota allocation |
| `crates/hagency-contract` | Server-engagement/project/agent/top-up types, policy, budget arithmetic and canonical digests | HTTP authentication, persistence or actual reservations |
| `crates/operations` | Native sessions, coordinator decisions, Inbox, execution projections, notifications and durable workflows | Actual Codex/Claude calls or runtime quota accounting |
| `crates/backend` | One listener, mounted components, fleet pairing, connection proof, room preparation, account integration and existing admin APIs | A second Node Operations process |
| `crates/frontend` / Rinx | Web administration and native project management | Authority inferred from UI state |
| hagency-rs | Resource ownership, actual reservations, provisioning, Codex/Claude execution, usage and receipts | Resource authority inferred from Matrix admin status |

There is still one server and three databases: hagency, palpo and pasion.
Operations shares the existing `public.hagency_admin_state` JSONB document in
hagency, without adding SQLite. A decision, audit record, command outbox,
notification intents and delivery rows commit together. Store clones share one
writer; the existing PostgreSQL advisory lock excludes a second process.
Cancelling an HTTP request cannot interrupt a commit already in progress.

## Scope of this migration

The source is pinned to Palpo PR #508 commit
`985c7242c2074b7cb0561c14c7c79dc6ed1a2bf7`. Contract and HTTP policy tests were
retained; SQLite-specific tests were replaced with PostgreSQL verification.
The client/Inbox direction from #506 and coordinator contract from #508 move
here. The older designated-Matrix-admin project approval model from #507 does
not introduce a second authorization system. Roles follow
[Rinx ADR 0011](https://github.com/hagency-org/Rinx/blob/main/docs/adr/0011-hagency-server-engagements.md).

| Capability | Current implementation |
| --- | --- |
| Canonical JSON, bounded budget arithmetic, immutable types and policy | `hagency-contract` |
| Borrowed Matrix identity, 15-minute native sessions, grants and disconnect | `operations::api` / `matrix` |
| Project/agent/top-up requests, approve/reject, seen/snooze | `operations::workflow` / `intents` |
| Role-scoped pagination, unknown/stale usage, distinct approval/execution | `operations::views` / `updates` |
| Durable outbox, generations/revisions, exact replay and execution receipts | `operations::workflow` / `outbound` / `updates` |
| Private My Actions rooms, idempotent messages, bounded reminders/retries | Optional `operations::notifications` worker |
| Web Inbox | `/hagency/inbox`; existing Padmin pages remain |
| Shared web/native business adapter | `POST /api/operations/call` |
| Existing fleet registration, pairing, connection proof, relay/poll/ACK and retirement | Backend admin modules, using the shared store |

Neither `hagency-contract` nor `hagency-operations` depends on Palpo internals.
The main backend embeds Palpo solely to provide MatrixServer. It does not run
Palpo's Node web-admin or open its Operations SQLite database. Upstream cleanup
is proposed separately in draft [PR #512](https://github.com/palpo-im/palpo/pull/512):
remove Node web-admin and its dedicated CI, retaining generic Matrix/App Service
administration APIs. Palpo main still contains the old application; merge only
after publishing the replacement and accepting consumer/state migration.
The business crates proposed in #508 have not landed in Palpo main.

## Identity, authority and execution

Pasion login produces a Matrix access token. Web calls retain cookie,
same-origin and CSRF checks. Native clients use a Matrix bearer to establish a
short-lived session and revalidate Matrix identity on every call. Native
endpoints reject browser Origin/Cookie contexts; browsers use the host adapter.

Matrix administration does not confer coordinator authority. A trusted
projection identifies the resource owner and delegated coordinator. The
coordinator approves projects; agent/top-up decisions also allow an authorized
resource owner. Self-approval needs explicit policy. Two server engagements on
the same homeserver cannot exchange authority. A legacy single-agent allocation
engagementId is not a server-engagement ID. The current delivery mapping requires
serverEngagementId to match its transport fleet/profile ID.

Approval records a verdict and command, not execution. A transport ACK confirms
custody only. Execution receipts and current observations advance pending,
provisioning and ready states. The runtime must recheck authority and parent
capacity in its actual reservation transaction; UI numbers cannot create
capacity. Unreported usage remains unknown, old observations remain stale and
top-ups cannot silently increase a parent grant.

## API compatibility

The canonical native API is:

```text
POST /_hagency/miniapp/v1/session
POST /_hagency/miniapp/v1/call
POST /_hagency/miniapp/v1/disconnect
appId: im.hagency.operations
services: hagency.inbox.*, hagency.projects.list, hagency.requests.list,
          hagency.intent.new, hagency.session.open, hagency.session.disconnect
```

`/_palpo/miniapp/v1/`, `im.palpo.operations` and `palpo.*` remain compatibility
aliases for a gradual Rinx switch. Moving crates does not change command wire
versions, serialized IDs or canonical digests. Only implemented services are
granted; missing services in the old manifest are not advertised as available.
The web adapter takes `{ "service": "hagency.inbox.list", "args": { "view": "all" } }`.
Mutations use expectedRevision and a stable commandId. An app cannot select its
authenticated actor.

## Authority import

Online association approval, owner-issued delegation and client room
preparation were incomplete in upstream #508. Moving code does not turn old
admins, old requests or hostnames into coordinators. At this stage an operator
imports a reviewed projection offline, with the server stopped:

```sh
just import-authority /absolute/path/reviewed-authority.json
# Another configuration:
just import-authority /absolute/path/reviewed-authority.json config/docker/hagency.toml
```

The command uses hagency's database, validates server/IDs/revisions/bindings and
exits without starting Palpo/Pasion. A running server's advisory lock refuses
this import. JSON contains engagements, resources and projects maps; see the
schema in [workflow tests](../crates/operations/tests/workflows.rs). Test fixtures
are not evidence of a real owner delegation. Preserve tombstones and monotonic
revisions instead of deleting/readding authority to reset its history.

Existing PostgreSQL fields, credentials, legacy requests, delivery leases and
unknown extensions are preserved. New workflows use `rustWorkflows`. This is
incremental preservation, not automatic semantic conversion of old Inbox
records or an importer for Node SQLite. Existing connections keep their former
workflows. Coordinator workflows need explicit authority and a compatible
hagency-rs runtime, followed by separate acceptance.

## Optional Matrix notifications

Add to hagency.toml:

```toml
[action_notifications]
bot_mxid = "@notifications:palpo.instance"
token_file = "../../secrets/notifications-token"
```

Use an ordinary Pasion/Matrix account's access token, without another business
admin account or admin token. Paths are relative to hagency.toml. Protect the
file and restart the server after rotating the token. Omitting this section
disables delivery; Inbox remains available.

The worker creates private, non-federated My Actions rooms, allowing only the
bot and recipient to join/be invited. It rechecks bindings, history visibility
and state permissions. Notices contain generic text and an opaque action
reference, without definitions, credentials or approval instructions. Decisions
happen in Inbox. The initial notice can be followed by at most three reminders
(1 hour, 1 day, 2 days). Seen/snooze, role changes and newer revisions suppress
old reminders. A lost reply retries the same Matrix transaction ID; errors use
bounded backoff.

## Validation and follow-up

```sh
just check-operations
cargo test --locked -p hagency-server
cargo check --locked -p hagency-frontend --target wasm32-unknown-unknown
```

Use an empty dedicated HAGENCY_TEST_DATABASE_URL for
`cargo test --locked -p hagency-operations postgres_shared_writer -- --ignored`.

Runtime consumers should import hagency-contract from this repository, and
Rinx should adopt the canonical namespace. Complete online association,
delegation issuance, legacy reconciliation, real hagency-rs provisioning/chat
and actual Codex/Claude quota execution still require cross-project acceptance.
Local fixture tests do not establish those workflows. URL-preview configuration
from Palpo #504 remains a Matrix concern in Palpo, not a Hagency business module.
