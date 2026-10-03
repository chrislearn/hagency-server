# hagency-server

Palpo Matrix homeserver and Palpo web-admin in **one Rust process and one HTTP
listener**. The application uses Rust, Tokio, Salvo, Diesel/diesel-async and
PostgreSQL, matching Palpo's backend. No Node server runs in production.

The existing HTML, JavaScript and CSS frontend is unchanged. It is embedded in
the binary by default and served at `/`. Matrix endpoints remain at `/_matrix`,
Palpo administration at `/_palpo`, discovery at `/.well-known/matrix`, and the
web-admin backend at `/api`. `/healthz` reports process HTTP availability.

## Architecture

```text
hagency-server (one process, one port)
├─ MatrixServer: Palpo initialization, Matrix/admin/discovery routes and workers
├─ Rust web-admin: sessions, Fleet/Agent, project requests, account approvals
├─ outbound relay: durable transactions, leases, ACKs and published snapshots
└─ unchanged web-admin assets
           │
           └─ PostgreSQL: Palpo tables + public.hagency_admin_state
```

Palpo is linked as a Rust library, not launched as a separate process or reached
through an external reverse proxy. Its existing authentication and operations
remain authoritative. For web-admin operations, the Rust adapter calls the
mounted Matrix/admin API over the fixed loopback origin in the **same process**.
This preserves actual Matrix authorization and App Service semantics. Internal
URLs are derived from `listen`; public transport URLs from `public_origin`.

The embedded Palpo API currently permits one MatrixServer per process because
Palpo's configuration and pools are singletons. The host owns Tokio, tracing,
HTTP shutdown and TLS termination. Run with 8 MiB Tokio worker stacks as this
binary does. Matrix background workers stop with the runtime.

## Development

Requirements: Rust >=1.94, PostgreSQL client library (`libpq`), and Docker for
the PostgreSQL service. Node is used only for the optional HTTP contract test.

```sh
python3 scripts/init-config.py --dev
docker compose up -d postgres
# On a new database, bootstrap an administrator as described below first.
python3 scripts/dev.py
```

Open `http://127.0.0.1:8088`. This development config uses
`localhost:8088` as the stable Matrix server name. Rust changes compile and then
gracefully restart the process. A compilation failure keeps the last working
server running. HTML/JS/CSS are read directly from `public/`; refreshing the
browser sees edits immediately without rebuilding or restarting Rust. Tokens in browser sessions remain memory-only,
so a server restart requires signing in again.

For Palpo source development, use a local checkout with the MatrixServer API:

```sh
python3 scripts/dev.py --palpo-source /absolute/path/to/palpo-checkout
```

The script writes a local Cargo patch under ignored `.run/`, watches the local
Palpo sources, and rebuilds against them. It does not modify the upstream
checkout or require a rebuilt Docker image. Cargo may update its lockfile when
switching between the pinned Git dependency and a local patch; return to the
normal dependency with `cargo update -p palpo` after removing the override.

## Configuration and initial administrator

One TOML file contains host settings and a `[matrix]` section with Palpo's
ServerConfig. `--check-config` validates it without starting a server. Embedded
Palpo ignores its `listeners` field; `listen` controls the single host listener.
Relative asset/config/data/media paths are resolved against the config file.
Without an explicit `[matrix.keypair]`, a signing key is generated once and
persisted in `data_dir/matrix-signing-key.json` (mode 0600). Back up that directory
and PostgreSQL together. `server_name` is an identity and must not be changed
when reusing a database.

Registration is disabled by default. For a new database, create an administrator
with a password file, then start the same application:

```sh
# Put a strong password in a protected file named secrets/admin-password.
cargo run -- --config config.dev.toml --bootstrap-admin admin \
  --bootstrap-password-file secrets/admin-password
```

Bootstrap refuses to modify an existing account. Use the full Matrix ID
`@admin:localhost:8088` to sign in. Stop a running development process before
using bootstrap; there must be one server process per database. Subsequent
starts omit the bootstrap options.

Optional `account_config` accepts the original web-admin JSON fields:
`botMxid`, `botToken`, `adminToken`, `approvers`, `passwordKey`, `registrationToken`.
The worker validates a private invite-only administrator room, checks the exact
approval event and the approver's current authority, and creates an ordinary
Matrix account. Pending passwords use AES-256-GCM; public APIs, audit records
and approval cards contain no password or access token. Palpo registration must
be enabled with the corresponding registration token for this optional flow.
`retirement_admin_token_file` can separately supply the server-only credential
used for Hagency final-allocation retirement.

## Compose deployment

```sh
python3 scripts/init-config.py --origin https://palpo.instance \
  --server-name palpo.instance
docker compose up -d postgres
docker compose build server
```

This creates a protected config and `.env` with a generated database password.
The first command prepares configuration without starting anything; the next two
prepare PostgreSQL and the image for administrator bootstrap below. Compose deploys
PostgreSQL plus the single hagency-server process. The Docker build uses one
compiler job and a lower optimization level for the large Palpo crate to reduce
peak memory; its crypto/HTTP/database dependencies retain full optimization.
Put the loopback-exposed
HTTP listener behind your HTTPS proxy. Preserve the public Host header and
forward `/`, `/api`, `/_matrix`, `/_palpo`, `/.well-known/matrix` and `/healthz`
to port 8088. Browser/admin/outbound clients use the configured public origin;
Palpo's App Service relay uses the derived internal origin. No separate
web-admin origin, port or relay URL needs configuration.

The entrypoint copies the mode-0600 mounted configuration into a private runtime
directory before dropping to UID 10001. The Rust process runs without root.
Generated production paths are absolute; use absolute paths for optional mounted
account configuration and token files, readable by UID 10001.

For bootstrap in Compose, after PostgreSQL is healthy:

```sh
docker compose run --rm --service-ports \
  -v "$PWD/secrets/admin-password:/app/bootstrap-password:ro" server \
  --config /app/config.toml --bootstrap-admin admin \
  --bootstrap-password-file /run/hagency/bootstrap-password
```

Stop that initial process after creating the account, then use
`docker compose up -d server`. For an existing initialized deployment,
`docker compose up -d --build` is sufficient. Bootstrap refuses an existing account. Do not add
a shared default administrator password to the image or config.

## Behavior retained in the Rust port

- Matrix password login; opaque HttpOnly/SameSite cookies, CSRF, Origin/Host
  checks, bounded bodies, rate limits, live identity/admin checks and deadlines.
- Fleet authorization, verified installation, collision/drift checks,
  owner-only credential delivery, pause/resume/final revoke and audit.
- Managed Agent identity creation, immutable profile update, observed membership
  and verified retirement, including Hagency final-allocation retirement.
- Exact Matrix probe receipts, generation-bound outbound connection proofs,
  project/private approval rooms, permission checks, source-bound requests and
  actual admission/status observations.
- Durable outbound transactions, ordered lanes, leases, ACK tombstones,
  bounded capacity, credential rotation, migration and replay, immutable update
  sequences, current snapshots and stale/unknown readiness rejection.
- Optional account approval worker with private receipt, encrypted pending
  password, authenticated verdict, registration recovery and result notices.

The PostgreSQL document and delivery queue commit atomically. A session-level
advisory lock fences the process for the lifetime of its dedicated connection.
A database outage fails writes; restart the process to recover that connection.
The current document store follows the original single-server/single-writer
model, with bounded queue capacity. It is not a clustered scheduling system.

This phase integrates Palpo and web-admin. Pasion/padmin and the project-first
Rinx client are subsequent work. The existing web-admin frontend still uses
Matrix password login; OAuth UI has not been substituted. Existing SQLite admin
databases are not automatically imported: use a fresh database for this version
until a separately validated migration is provided.

## Validation

```sh
cargo fmt --check
cargo check --all-targets --locked
cargo test --locked
cargo build --example admin_contract_server --locked
node tests/http-contract.mjs
# If target artifacts are elsewhere:
CONTRACT_SERVER=/absolute/path/to/admin_contract_server node tests/http-contract.mjs
```

The HTTP contract harness starts the Rust adapter and a controlled Matrix
fixture. It covers the unchanged assets, authentication and isolation,
Fleet/Agent lifecycle, project admission, outbound proof/ACK/rotation, exact
request bindings, final allocation retirement and approved account provisioning.
It does not connect to a model. PostgreSQL persistence/lock tests use an
explicit dedicated `HAGENCY_TEST_DATABASE_URL`:

```sh
HAGENCY_TEST_DATABASE_URL=postgres://... cargo test --lib postgres_restart -- --ignored
```

`tests/integration.mjs` verifies the real integrated binary against a dedicated
PostgreSQL database, including administrator login, Matrix discovery, Fleet
registration, real relay delivery, restart recovery and persistent signing keys.
The integration script refuses a database containing any application tables.
To run it, install `psql`, create a new empty dedicated database, then:

```sh
HAGENCY_TEST_DATABASE_URL=postgres://... node tests/integration.mjs
# Build an image and verify an isolated deployment, including restart:
docker build -t hagency-server:integration-check .
node tests/docker-smoke.mjs
```

The Docker smoke test creates a unique Compose project and removes its own
containers/volumes afterward. See `VALIDATION.md` for executed checks and limits.

Palpo embedding change: [upstream PR #505](https://github.com/palpo-im/palpo/pull/505).
