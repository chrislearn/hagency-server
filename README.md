# hagency-server

Palpo Matrix homeserver, Pasion authentication and Palpo web-admin in **one Rust
process and one HTTP listener**. The application uses Rust, Tokio, Salvo, Diesel/diesel-async and
PostgreSQL, matching Palpo's backend. No Node server runs in production.

The Cargo workspace follows Pasion's backend/frontend organization:

```text
crates/
├─ backend/   # hagency-server: embedded Palpo, Pasion and Hagency APIs
└─ frontend/  # hagency-frontend: Dioxus/WASM, based on copied Padmin source
xtask/       # independent development/configuration tools
resources/   # generated frontend and Pasion assets, ignored by Git
```

The frontend combines Padmin's Matrix administration screens with the previous
web-admin workflows: projects, agent resource selection and requests, provider
authorization and pairing, agent identities, account approvals and audit history.
It is served at `/` by the Rust backend. Matrix endpoints remain at `/_matrix`,
Palpo administration at `/_palpo`, discovery at `/.well-known/matrix`, and
Hagency APIs at `/api`. Pasion remains mounted at `/_pasion/`.
`/healthz` reports process HTTP availability.

## Architecture

```text
hagency-server (one process, one port)
├─ MatrixServer: Palpo initialization, Matrix/admin/discovery routes and workers
├─ Rust web-admin: sessions, Fleet/Agent, project requests, account approvals
├─ outbound relay: durable transactions, leases, ACKs and published snapshots
├─ PasionServer: OAuth/OIDC, account UI/API and workers at /_pasion/
└─ integrated Padmin + Hagency Dioxus/WASM assets
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

Requirements: Rust >=1.94, [just](https://github.com/casey/just), PostgreSQL client library (`libpq`), and Docker for
the PostgreSQL service. Embedded Pasion also needs the wasm32-unknown-unknown
target and Dioxus 0.7.5 assets; resource preparation downloads the matching
Dioxus CLI when necessary. Git and curl are used for fetching build tools/source.
Node is used during frontend build and HTTP tests. Python is not required.

`just --list` shows the commands. Just orchestrates them; an independent Rust
`xtask` implements private configuration generation, asset preparation and the
development watcher without compiling the server to run these tools.

```sh
just init-dev
just db-up
just prepare-pasion
just prepare-frontend
# On a new database, bootstrap an administrator as described below first.
just dev
```

Open `http://127.0.0.1:8088`. This development config uses
`localhost:8088` as the stable Matrix server name. Rust changes compile and then
gracefully restart the process. A compilation failure keeps the last working
server running. Frontend Rust/CSS changes rebuild the WASM assets, which the
running host serves immediately; refresh the browser to load them. Frontend-only
changes do not restart the backend. No Docker image rebuild is required.
Browser access/refresh tokens remain in memory, so a full page reload requires
signing in again. SPA navigation preserves the session. Host restarts can
rebind the existing in-memory Matrix token to a new Hagency cookie session.

For Palpo source development, use a local checkout with the MatrixServer API:

```sh
just dev --palpo-source /absolute/path/to/palpo-checkout
```

The development tool writes a local Cargo patch under ignored `.run/`, watches the local
Palpo sources, and rebuilds against them. It does not modify the upstream
checkout or require a rebuilt Docker image. Cargo may update its lockfile when
switching between the pinned Git dependency and a local patch; return to the
normal dependency with `cargo update -p palpo` after removing the override.

The copied Padmin also contains optional `palpo_admin` sidecar screens for
maintenance, scheduled commands and notifications. That separate sidecar is
not embedded here; its menu stays disabled instead of sending requests to
nonexistent Palpo endpoints. Matrix user/room/media/registration/App Service
administration uses the mounted Palpo APIs directly.

## Unified frontend authentication

The host publishes nonsecret runtime settings at `/config.json`. With
`[hagency].delegate_matrix_auth = false` in Pasion's config, the frontend signs
in through Palpo's native Matrix password login. With delegation enabled, it
uses Pasion OAuth Authorization Code + PKCE instead. Member login requests
Matrix client/device scopes; the separate administrator login also requests
Palpo/Pasion administrative scopes, which Pasion grants only to administrators.
The host provisions the reserved public frontend OAuth client and derives its `/oauth/callback` URL;
no client secret or separate frontend configuration is required.

Both modes exchange the resulting Matrix token at `POST /api/login/token`.
The backend validates `whoami` and the live Matrix administrator permission
before issuing its HttpOnly cookie and CSRF token. Ordinary members see projects,
Agent requests and their own Hagencys; Matrix administrators also see Padmin and
Hagency administration. Pasion administration additionally requires a Pasion
OAuth token, administrative scope and Pasion administrator permission, checked
by Pasion's own endpoints. Pasion's administrator flag and Palpo's Matrix
administrator flag remain separate: a full server administrator needs both;
this frontend migration does not promote accounts automatically.
In native authentication mode, the Pasion account center remains separately
available at `/_pasion/`; its registration passwords are validated by Pasion and
do not enable Palpo native password login. Account approval requests are offered
only when configured.

Frontend source provenance and AGPL licensing are recorded in `NOTICE` and
`crates/frontend/LICENSE` (Padmin commit `83d4567470ada808b89914aa0c786cdc3a7ac89a`).

## Configuration and initial administrator

Configuration is split into files owned by each component:

```text
config/
├─ examples/       # tracked, native configuration templates
│  ├─ hagency.toml
│  ├─ palpo.toml
│  └─ pasion.toml
├─ dev/            # generated, ignored, mode 0600 files
│  ├─ hagency.toml
│  ├─ palpo.toml
│  └─ pasion.toml
└─ docker/         # generated, ignored, mode 0600 files
   ├─ hagency.toml
   ├─ palpo.toml
   └─ pasion.toml
```

Start with `--config config/dev/hagency.toml` (the CLI default). Hagency owns the
listener, admin connection and data directory, and references the other files:

```toml
palpo_config = "palpo.toml"
pasion_config = "pasion.toml"
```

Palpo uses its native top-level ServerConfig, including `[db]`, `[storage]` and
`[well_known]`; there is no `[matrix]` wrapper. Pasion uses its native sections,
including `[database]`, `[account]`, `[email]`, `[[clients]]` and
`[[upstream_oauth2.providers]]`; there is no `[pasion.settings]` wrapper. Its
additional `[hagency]` section contains `resources_dir` and
`delegate_matrix_auth` for embedding.

| File and setting | Database | Contents |
| --- | --- | --- |
| `hagency.toml`: `database_url` | `hagency` | web-admin/Fleet/project state and outbound queue |
| `palpo.toml`: `db.url` | `palpo` | Matrix users, rooms, events and homeserver state |
| `pasion.toml`: `database.uri` | `pasion` | Accounts, OAuth/OIDC tokens and sessions |

Database names must be distinct. Omit `pasion_config` to disable Pasion and use
only the independent Hagency and Palpo databases. Paths resolve relative to the
file that declares them, including component references, media and supported
native secret-file references. The development watcher monitors all three files,
including references outside the main configuration directory.

`--check-config` loads/checks all referenced files without starting a server;
`--list-config-files` prints their paths as JSON without connecting to databases.
Hagency's listener replaces Palpo/Pasion listener configuration in embedded mode.
Without an explicit `[keypair]` in `palpo.toml`, a signing key is generated once and
persisted in `data_dir/matrix-signing-key.json` (mode 0600). Back up that directory
and PostgreSQL together. `server_name` is an identity and must not be changed
when reusing a database.

Registration is disabled by default. For a new database, create an administrator
with a password file, then start the same application:

```sh
# Put a strong password in a protected file named secrets/admin-password.
just run --config config/dev/hagency.toml --bootstrap-admin admin \
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
just init-docker --origin https://palpo.instance \
  --server-name palpo.instance
just db-up
just docker-build
```

This creates `config/docker/{hagency,palpo,pasion}.toml` and a protected `.env`
with a generated database password. Development generation similarly creates
`config/dev/`; `--output-dir` can select another directory. The generator refuses
to overwrite an existing directory.
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

Compose mounts `config/docker/` at `/app/config/`. The entrypoint copies this
configuration tree into `/run/hagency/config/` (directories mode 0700, files mode
0600) before dropping to UID 10001. References between component files and
secret files inside this tree stay relative. The Rust process runs without root.
Generated production paths are absolute; use absolute paths for optional mounted
account configuration and token files, readable by UID 10001.

For bootstrap in Compose, after PostgreSQL is healthy:

```sh
docker compose run --rm --service-ports \
  -v "$PWD/secrets/admin-password:/app/bootstrap-password:ro" server \
  --config /app/config/hagency.toml --bootstrap-admin admin \
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

This phase integrates Palpo, Pasion, Padmin and Hagency web administration.
The project-first Rinx client is subsequent work. Existing SQLite admin databases
are not automatically imported: use a fresh database for this version until a
separately validated migration is provided.

## Validation

```sh
just check-tools
cargo fmt --check
cargo check --all-targets --locked
cargo test --locked
cargo build --example admin_contract_server --locked
CARGO_TARGET_DIR=.run/frontend-target cargo test --locked -p hagency-frontend
just check-tools
node tests/http-contract.mjs
# If target artifacts are elsewhere:
CONTRACT_SERVER=/absolute/path/to/admin_contract_server node tests/http-contract.mjs
```

The HTTP contract harness starts the Rust adapter and a controlled Matrix
fixture. It covers the Dioxus SPA/assets and token bridge, authentication and isolation,
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

Enable embedding by referencing `pasion.toml` through `pasion_config` in
`hagency.toml`. Generated profiles include that reference. Configure the Pasion
DB in `[database].uri` and assets in `[hagency].resources_dir` in its own file.
Native email, SMS, account registration, clients, upstream OAuth providers, pool
limits, rate limits and branding settings live directly in `pasion.toml`.
Networking/issuer, Matrix connections, templates, storage and key ownership are
still derived by Hagency; those sections cannot override the host wiring. Native
database pool and TLS settings are retained. All three databases remain separate.

For native development, build Pasion's Dioxus WASM frontend/resources once:

```sh
just prepare-pasion
just dev --pasion-source /path/to/pasion --palpo-source /path/to/palpo
```

`just prepare-pasion` uses the pinned source by default; `--source` takes a local
checkout. It installs Rust's `wasm32-unknown-unknown` target if needed.
The Rust tool reuses `dx` 0.7.5 or fetches a matching, checksum-verified CLI into
`.run/tools/` without replacing a globally installed version.
The development watcher rebuilds edited Pasion backend/frontend code and
copies templates/translations/policies, then gracefully replaces the server.
No image rebuild is involved. Docker builds and packages those resources as
part of its image.

For local registration tests, enable `[account].password_registration_enabled`
and set the following native options in `config/dev/pasion.toml`:

```toml
[experimental]
fixed_verification_code = "123456"
[email.provider]
type = "blackhole"
[sms.provider]
type = "blackhole"
```

Email and SMS contact verification then store `123456` and skip outbound
verification notifications. Enter that code on the verification page; resend
once if a registration started before this setting was enabled. The watcher
reloads this file. Leave the fixed-code option unset outside local tests.

Mounting Pasion and switching Matrix login are independent choices.
`[hagency].delegate_matrix_auth = false` in `pasion.toml` preserves Palpo's existing native login.
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

Set `hagency.toml` → `database_url` to `/hagency`, `palpo.toml` → `[db].url`
to `/palpo`, and `pasion.toml` → `[database].uri` to `/pasion`. Keep the same Matrix server
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

### Moving the previous single configuration into component files

For an existing three-database configuration, keep the database URLs, server
identity, key/media paths and authentication settings unchanged while splitting:

1. Put host settings into `hagency.toml`; replace its inline component sections
   with `palpo_config` and optional `pasion_config` file references.
2. Move `[matrix]` into `palpo.toml` at the top level; remove the `matrix.` prefix
   from its subsection names, such as `[matrix.db]` → `[db]`.
3. Move `[pasion].database_url` to `[database].uri` in `pasion.toml`. Move
   `resources_dir`/`delegate_matrix_auth` into its `[hagency]` section and unwrap
   `[pasion.settings.*]` into the corresponding native sections.
4. Adjust relative paths to each new file location, or keep absolute paths;
   check with `hagency-server --config <path>/hagency.toml --check-config`.

The previous private `config.dev.toml`/`config.docker.toml` files are not rewritten
or imported automatically. Preserve them during the move. If their databases
are still combined, complete the database split above before using the new files.
