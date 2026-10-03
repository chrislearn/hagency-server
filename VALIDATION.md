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
not exercise Matrix federation, every Matrix API, OAuth, or the complete optional
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
