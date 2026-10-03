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
