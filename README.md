# hagency-server

Palpo Matrix homeserver, Pasion authentication and Palpo web-admin in **one Rust
process and one HTTP listener**. The application uses Rust, Tokio, Salvo, Diesel/diesel-async and
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
├─ PasionServer: OAuth/OIDC, account UI/API and workers at /_pasion/
└─ unchanged web-admin assets
           │
           └─ PostgreSQL service
              ├─ hagency: management state + durable outbound queue
              ├─ palpo: Matrix homeserver tables
              └─ pasion: authentication tables
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
the PostgreSQL service. Embedded Pasion also needs the wasm32-unknown-unknown
target and Dioxus 0.7.5 assets; the preparation script downloads the matching
Dioxus CLI when necessary. Node is used during frontend build and HTTP tests.

```sh
python3 scripts/init-config.py --dev
docker compose up -d postgres
rustup target add wasm32-unknown-unknown
python3 scripts/prepare-pasion.py
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
ServerConfig. The full deployment uses three databases on one PostgreSQL service:

| Configuration | Database | Contents |
| --- | --- | --- |
| Top-level `database_url` | `hagency` | web-admin/Fleet/project state and outbound queue |
| `matrix.db.url` | `palpo` | Matrix users, rooms, events and homeserver state |
| `pasion.database_url` | `pasion` | Accounts, OAuth/OIDC tokens and sessions |

The top-level `database_url` is required; the admin store never falls back to
Palpo's connection. Database names must be distinct even when differently
spelled hostnames refer to the same PostgreSQL server. Omitting `[pasion]` leaves
the two independent Hagency and Palpo databases.

`--check-config` validates the configuration without starting a server. Embedded
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
forward `/`, `/api`, `/_matrix`, `/_palpo`, `/_pasion/`, `/.well-known/matrix` and `/healthz`
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

This phase integrates Palpo, Pasion and web-admin. padmin and the project-first
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

`tests/integration.mjs` verifies the real integrated binary against dedicated
Hagency and Palpo PostgreSQL databases, including administrator login, Matrix
discovery, Fleet registration, real relay delivery, restart recovery and persistent signing keys.
The integration script refuses either database if it contains application tables.
To run it, install `psql`, create two empty dedicated databases, then:

```sh
HAGENCY_TEST_DATABASE_URL=postgres://.../hagency \
PALPO_TEST_DATABASE_URL=postgres://.../palpo node tests/integration.mjs
# Build an image and verify an isolated deployment, including restart:
docker build -t hagency-server:integration-check .
node tests/docker-smoke.mjs
```

The Docker smoke test creates a unique Compose project and removes its own
containers/volumes afterward. See `VALIDATION.md` for executed checks and limits.

Palpo embedding change: [upstream PR #505](https://github.com/palpo-im/palpo/pull/505).

### Embedded Pasion

Pasion now shares the Rust process and listener with Palpo and web-admin.
The embedding API and subpath support are proposed upstream in
[Pasion PR #102](https://github.com/meldry-com/pasion/pull/102):

| Component | URL |
| --- | --- |
| Existing web-admin | `/` |
| Matrix client/federation/admin APIs | Existing Matrix/Palpo paths |
| Pasion account UI | `/_pasion/` and `/_pasion/login` |
| OIDC discovery | `/_pasion/.well-known/openid-configuration` |
| OIDC issuer | `<public_origin>/_pasion/` |
| OAuth token / JWKS | `/_pasion/oauth2/token`, `/_pasion/oauth2/keys.json` |

No separate Pasion daemon, public IP, Node server or reverse-proxy rewrite is
needed. Pasion background tasks call Palpo's protected MAS APIs through the
same listener's loopback address. The host creates/persists OAuth signing keys,
cookie encryption key and the shared Matrix secret in mode-0600
`data/pasion-secrets.json`. Keep this file together with the database backups.

All three components use **separate databases** on one PostgreSQL service.
Compose creates `hagency` through `POSTGRES_DB`; `deploy/databases.sql` creates
`palpo` and `pasion` when initializing a fresh volume. PostgreSQL initialization
scripts do not rerun for existing volumes. See the database split instructions
below before upgrading an old combined Hagency/Palpo database.

Enable embedding by adding `[pasion]` to the single host TOML, with
`database_url` and `resources_dir`. The generated development/deployment
configs include this section. Native Pasion settings (email, SMS, account
registration, clients, upstream OAuth providers, rate limits, branding) belong
under `[pasion.settings]`; host-managed HTTP, database, Matrix, keys, templates
and storage cannot be overridden there. Existing configurations without a
`[pasion]` section remain valid after adding the independent top-level
`database_url`.

For native development, build Pasion's Dioxus WASM frontend/resources once:

```sh
python3 scripts/prepare-pasion.py
python3 scripts/dev.py --pasion-source /path/to/pasion --palpo-source /path/to/palpo
```

`prepare-pasion.py` uses the pinned source by default; `--source` takes a local
checkout. It requires Rust's `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`).
The script reuses `dx` 0.7.5 or fetches a matching, checksum-verified CLI into
`.run/tools/` without replacing a globally installed version.
The development watcher rebuilds edited Pasion backend/frontend code and
copies templates/translations/policies, then gracefully replaces the server.
No image rebuild is involved. Docker builds and packages those resources as
part of its image.

Mounting Pasion and switching Matrix login are independent choices.
`pasion.delegate_matrix_auth = false` preserves Palpo's existing native login.
Set it to `true` **after preparing Pasion accounts** to advertise MSC3861 and
delegate password login/token introspection to the mounted service. Host
configuration derives the public issuer and internal endpoints automatically.
Existing Palpo passwords are not copied into Pasion. Registration is disabled
by default; configure an upstream provider or explicitly enable Pasion
registration. This stage does not add legacy Matrix SSO redirects; use OIDC
clients or the delegated password flow. Web-admin keeps its existing UI and
uses the Matrix login endpoint, so delegated password login works there too.
Legacy web-admin account-approval workflows require native auth and are
rejected when combined with Pasion delegated registration.

The combined distribution includes AGPL-3.0-only Pasion and uses that license.
The original Apache-2.0 license/notices for Palpo-derived code are retained in
`LICENSE.Apache-2.0` and `NOTICE`.

### Upgrading the former combined database

This change does not automatically rename databases, move existing data or
rewrite private configuration files. For a former deployment with Matrix and
admin tables together in `hagency`, stop all server processes and back up that
database and the persisted key/media directory first. The following commands
assume the original Compose setup, no existing `palpo` database, and an existing
`public.hagency_admin_state` table. Run them only after stopping any native
server connected to those databases as well.

```sh
docker compose stop server
umask 077
mkdir -p backups/db-split
docker compose exec -T postgres pg_dump -U hagency -Fc hagency > backups/db-split/hagency-before-split.dump
docker compose exec -T postgres pg_dump -U hagency --no-owner --no-privileges -t public.hagency_admin_state hagency > backups/db-split/admin.sql
# Stop here if either backup fails.
docker compose exec -T postgres psql -U hagency -d postgres -v ON_ERROR_STOP=1 \
  -c 'ALTER DATABASE hagency RENAME TO palpo;' \
  -c 'CREATE DATABASE hagency OWNER hagency;'
docker compose exec -T postgres psql -U hagency -d hagency --single-transaction -v ON_ERROR_STOP=1 < backups/db-split/admin.sql
```

Set the top-level `database_url` to `/hagency`, change `matrix.db.url` to
`/palpo`, and keep `pasion.database_url` at `/pasion`. Keep the same Matrix server
name and key/media directory. If Pasion was never enabled, create its database
once before enabling it. The configuration generator intentionally refuses to
overwrite existing files.

After confirming the admin restore succeeded, remove its original table from
`palpo` so it contains only Matrix data:

```sh
docker compose exec -T postgres psql -U hagency -d palpo -v ON_ERROR_STOP=1 -c 'DROP TABLE public.hagency_admin_state;'
```

The server can then be restarted with the updated configuration and image.
Existing Matrix accounts and Fleet/queue state are retained by the rename and
admin-table copy; existing Pasion data stays in its database.
