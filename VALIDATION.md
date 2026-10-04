# Executed validation

Validated locally on 2026-10-03 with Rust 1.98.1, PostgreSQL 18.6 and Node 26.9.
The final Cargo dependency is the public Palpo Git revision
`c8568d9844a6be0a3172d98d7c9810e3a1f7521c` ([PR #505](https://github.com/palpo-im/palpo/pull/505)).

## Passed

- Palpo library/binary compilation and library suite: 244 passed, 17 opt-in
  database regressions ignored. Includes a mounted-router ownership regression.
- `cargo fmt --check`, locked all-target compilation and
  `cargo clippy --all-targets --locked --no-deps -- -D warnings`.
- `cargo test --locked`: atomic rollback and two unified-config/signing-key tests.
- Opt-in PostgreSQL test: persistent delivery state, reopen/restart, single writer
  advisory lock and rejection of a second writer.
- Ten HTTP contract groups against a controlled Matrix fixture: exact frontend
  bytes; session/Host/Origin/CSRF/live authorization; owner credential isolation;
  Fleet install/idempotency/lifecycle; managed identity lifecycle; private project
  approval rooms and request source bindings; outbound lease/ACK/proof/rotation;
  resource admission and verified final-allocation retirement; ordinary account
  provisioning through private authenticated approval.
- Real integrated binary against an empty isolated PostgreSQL database:
  Matrix administrator bootstrap/login, homepage, Matrix versions and discovery,
  real App Service registration/representative creation, real event transaction
  delivery to the mounted relay, ACK and exact probe proof, graceful restart,
  retained users/Fleet/credentials/signing key and persisted delivery history.
- Original frontend HTML/JS/CSS compared byte-for-byte with Palpo's checkout.
- Development config `--check-config`, Python/Node/shell syntax and Compose
  configuration validation.

## Docker verification

The first fully optimized build exceeded the Docker VM's 8 GB memory budget.
The build now uses one compiler job and opt-level 1 for the large Palpo crate;
its dependencies retain full optimization.

- `docker build -t hagency-server:integration-check .` passed on Linux arm64.
  Final release build completed within the 8 GB VM memory budget.
- `node tests/docker-smoke.mjs` passed against that image: isolated PostgreSQL
  and Rust server, PID 1 runs as UID 10001, runtime configuration mode 0600,
  unchanged homepage, mounted Matrix APIs, actual administrator password login,
  graceful restart, retained account and identical persisted signing key.
- Test Compose project containers and named volumes were removed by the harness.

Image manifest: `sha256:acc80a21ba2a2b5772110463ec4faf1e4112609cd09f5c31a6e0d1c600d573d5`.
Image size reported by Docker: 359,121,160 bytes.

## Scope and limits

The contract fixture is intentionally controlled; it proves HTTP behavior and
state transitions, not interaction with a live Codex/Claude model runtime. The
real integration proves actual Palpo event delivery and restart recovery. It does
not exercise Matrix federation, every Matrix API, or the complete optional
account flow against a real approval bot. No existing development stack or user
database was stopped, reconfigured or migrated. Test containers/volumes are
isolated and removed after verification.

The adapter retains the original single-writer model. Browser sessions are
memory-only. Existing SQLite admin state is not automatically migrated. Palpo's
embedding API remains process-global, with one MatrixServer per runtime/process.

## Upstream CI baseline

Palpo's current main revision already fails its strict workspace Clippy job with
17 redundant-field diagnostics from existing Diesel-derived data structs:
[main workflow](https://github.com/palpo-im/palpo/actions/runs/37019841267).
PR #505 has the same diagnostics, outside this change. This is separate from the
passed downstream all-target Clippy check above. The library extraction also
exposed two stale documentation imports; these are corrected in a documentation
follow-up on the Palpo PR (`cargo test -p palpo --doc --locked`: 2 passed).
The server remains pinned to the runtime revision
used in all native and Docker integration tests, rather than an untested update.

## Embedded Pasion (2026-10-04)

Pasion is pinned to `b8333e1ee60c6362847d0f1890461b7af7547037`
([PR #102](https://github.com/meldry-com/pasion/pull/102)). It uses the same
Tokio/Salvo process and listener at `/_pasion/`, with its own database on the
shared PostgreSQL service. No existing development deployment was changed.

Executed checks:

- Pasion mounted-state isolation, SPA prefix, API schema base paths and legacy
  account redirects; loopback homeserver proxy bypass; template prefix tests.
- Standalone Pasion CLI/backend compilation and CI-pinned formatter.
- Dioxus 0.7.5 release WASM assets built with `--base-path /_pasion/`.
- Browser login and Security Center navigation; prefixed JS/WASM/CSS, no browser
  errors.
- Host configuration regression rejects matching database names even through
  differently spelled hosts, before writing keys or creating data directories.
- Original web-admin ten-group HTTP contract and actual Palpo/Fleet integration
  still pass with Pasion dependencies linked.

- Final pinned revision: `cargo build --locked` and `cargo test --all-targets
  --locked` passed (one atomic-state test and three configuration tests; one
  opt-in database test remains ignored in this invocation).
- Final native integration passed: same listener, prefixed discovery, SPA, JS,
  WASM and API schema; legacy account redirect; registration/browser login;
  background Palpo provisioning; delegated Matrix password login/introspection;
  identical signing/encryption keys across restart and graceful shutdown.

- Final host `cargo fmt --check` and strict all-target Clippy passed.

- Final Linux arm64 Docker build passed within the 8 GB VM budget. Backend
  release build took 8m50s with warmed dependency caches; Dioxus release assets
  also built inside their separate build stage.
- `HAGENCY_TEST_IMAGE=hagency-server:pasion-integration-check node
  tests/docker-smoke.mjs` passed: unchanged homepage, actual administrator login,
  same-port Matrix/Pasion discovery and account UI, UID 10001, mode-0600 runtime
  config and OAuth keys, graceful restart, retained administrator, unchanged
  Matrix signing key and OAuth JWKS. The harness removed its containers/volumes.

Pasion-integrated image manifest:
`sha256:a542c5c46cc2774fa3dedad5e1c382ac79ec6fefaebafc2cdbe787e8a4849496`.
Docker-reported size: 439,346,725 bytes. Native test PostgreSQL containers and
their temporary data were also removed.

The authentication integration verifies actual account registration, background
Palpo provisioning, delegated Matrix password login and token introspection.
It does not prove every OIDC authorization-code/PKCE or external-provider flow.
Delegated Matrix authentication is opt-in; existing Palpo passwords are not
automatically migrated into Pasion.

The upstream PR also includes a test-only discovery-cache isolation follow-up.
Coverage runs independent AppStates in one process and previously reused the
first issuer. The complete backend suite after that fix passed with an isolated
PostgreSQL database: 191 passed, 1 ignored. The host remains pinned to the
production revision validated above; this test-only follow-up retains production
caching behavior.

## Three independent databases (2026-10-04)

The earlier sections record the two-database baseline. The current configuration
requires an independent top-level `database_url` for Hagency and uses
`matrix.db.url` for Palpo and `pasion.database_url` for Pasion. Defaults are
`hagency`, `palpo`, `pasion` on one PostgreSQL service. Fresh Compose volumes
create all three; existing volumes/configurations require the documented split
procedure and are not altered by this change.

Executed checks:

- Locked native binary/example compilation, all-target tests, formatter and
  strict all-target Clippy passed. Pairwise database collisions (including
  different host spellings), missing Hagency URL, and invalid PostgreSQL URLs
  are rejected before initialization.
- Generated development and Compose configuration validated with the actual
  binary: three connection URLs and mode 0600, in isolated temporary directories.
- Ten-group unchanged HTTP contract passed.
- Actual Palpo/Fleet integration passed on separate Hagency and Palpo databases:
  the admin DB contains only `hagency_admin_state`; the Matrix DB contains its
  homeserver tables and no admin table. Actual event delivery/ACK/proof and
  retained users/Fleet/transport state after restart passed.
- Actual Pasion integration passed on three independent databases: registration,
  background Palpo provisioning, delegated Matrix login/introspection, prefixed
  frontend/API/discovery, stable keys and graceful restart/shutdown.
- Linux arm64 Docker image built and isolated Compose smoke test passed: three
  table sets in their respective databases, non-root UID, mode-0600 config/keys,
  unchanged homepage, native admin login, Matrix/Pasion endpoints, retained
  account and stable signing keys after restart.

Image manifest:
`sha256:78915ebed3329376292f5c5a7b16095bf364a66243aacfa3bca8a9932575b7bc`.
Docker-reported size: 439,336,859 bytes. All database/integration deployments use
isolated containers and temporary configurations; existing development stacks
and private configuration files were not changed.

## Component-owned configuration files (2026-10-04)

Current profiles are `config/dev/` and `config/docker/`, with separate
`hagency.toml`, `palpo.toml` and `pasion.toml`. Templates live in
`config/examples/`. Hagency references the two component files, Palpo keeps its
native ServerConfig sections, and Pasion keeps native sections plus `[hagency]`
embedding options. Database names and runtime ownership are unchanged.

Executed checks:

- Locked native binary/example compilation and all-target tests passed, including
  four configuration regressions: persistent keys, schema/host checks, three DB
  collision/missing URL checks, and paths resolved against independently located
  component files (media, registration token and password secret files).
- Strict all-target Clippy and formatter passed.
- Development and production generator checked in isolated directories: three
  valid files, mode 0600, directory mode 0700, explicit origin/Matrix identity,
  native file references, and refusal to overwrite existing output.
- `--list-config-files` returns all component references without database access;
  the development watcher includes referenced files outside the main directory.
- Ten-group HTTP contract passed with native Palpo configuration.
- Actual Palpo/Fleet delivery/ACK/proof and restart integration passed using
  separate host/Palpo config files and databases.
- Actual Pasion registration, Palpo provisioning, delegated Matrix login/token
  introspection, prefixed UI/API/discovery and stable keys across restart passed
  using three config files; native Pasion database pool settings are retained.
- New local development files were generated and checked without starting any
  application or changing existing databases. The previous private root-level
  configuration files were preserved.

- Final Linux arm64 image built successfully. The isolated Compose smoke test
  passed with the mounted component configuration directory: all three native
  files loaded, runtime directory mode 0700 and all three files mode 0600 under
  UID 10001, unchanged frontend, actual administrator login, Matrix/Pasion
  endpoints, separate database table sets, graceful restart and identical
  Matrix/OAuth signing keys. The test harness removed its containers/volumes.

Image manifest:
`sha256:c6efb58bc46828c5464a4a87b1442a3dd9014618b05f36c6e611508c17ad5ea7`.
Docker-reported size: 439,287,638 bytes. Native test PostgreSQL was also removed;
existing application stacks/databases were not restarted or migrated.

## Just commands and Rust development tools (2026-10-04)

The three Python tools have been replaced by an independent Rust `xtask` and a
root `justfile`. Configuration paths/schema, the server binary and frontend
behavior are unchanged. Docker asset preparation also uses the Rust tool and
does not install Python.

Executed checks:

- `just check-tools`: formatter, strict all-target Clippy and four tests passed.
  The tests cover private profile permissions, password URI escaping, separate
  database names, preservation of existing credentials/configuration, local
  resource copying, and checksum verification before installing Dioxus.
- The watcher was exercised with isolated compiler/server fixtures: compilation
  and configuration failures preserve the running process; recovery, external
  Palpo/Pasion config edits and local Palpo source edits trigger replacement;
  SIGTERM gracefully stops the child. Custom Cargo target directories and paths
  containing spaces are supported.
- Actual just recipes generated a temporary three-file development profile and
  validated it with the real server's `--check-config`; local Pasion resources
  were copied through `just prepare-pasion --skip-frontend`. Arguments containing
  spaces passed intact. Existing private configuration files were preserved.
- Just/server formatting, diff whitespace and Compose configuration checks passed.
- Linux arm64 image built successfully using the Rust resource preparation tool,
  including a real Dioxus WASM build. The isolated Compose smoke test passed:
  three component configurations and separate databases, non-root process,
  same-port web-admin/Matrix/Pasion, administrator login, stable keys and restart.
  The test removed its containers/volumes; existing services were not restarted.

Image manifest:
`sha256:35c0a571c3f33607d2ef6187aa35a67a420e50fdd9f57ea2246c0c324bae6b6d`.
Docker-reported size: 439,287,638 bytes. Python and the Rust development helper
are absent from the final runtime image.
