# Isolated database and independent filesystem restore evidence

[中文](BACKUP_RESTORE_VALIDATION.zh-CN.md) · [Recovery contract](RECOVERY.md)

On 2026-10-07, the actual embedded Hagency/Palpo/Pasion binary passed the optional recovery exercise in `scripts/test-agent-integration.py`. Run from the server repository:

```sh
HAGENCY_TEST_BACKUP_RESTORE=1 python3 scripts/test-agent-integration.py
```

This flag enables native Pasion DCR/PKCE/consent automatically. The final independent-filesystem run used this flag alone and passed; no model or tool runner was invoked. The script uses three random fixture databases, stops its sole server before all dumps, and restores all databases with `pg_dump --format=custom` / `pg_restore --exit-on-error` into three different new random databases. It snapshots the complete fixture data/media/config and restores them into a different private temporary directory. It changes only database URLs and local filesystem roots, then restarts one instance with the same issuer, homeserver, port and AS namespace/registration. Before restart it renames the source directory offline, making every original pathname unavailable. The recovered process uses its own log file and cannot silently read original identity or media files. It never overwrites a source database or runs two instances sharing the AS identity. Dumps and configuration stay private; secrets and dump contents are not printed. Both source and restored databases are dropped in `finally`; both successful source/offline and restored fixture directories were removed.

Verified before restart: the exact permanent Agent owner/puppet row, every stored command row (including its request digest), every AS transaction ID/digest, and the original sent reply row remained unchanged. After startup, all previous AS digests were retained and a new real startup AS event confirmed readiness. The AS registration, Matrix signing key, Pasion signing/encryption secrets and configured password-pepper file stayed byte-identical after restart. Every copied file was checked by SHA-256 and mode before configuration locations were adapted; required private key/config files retained private permissions. Hashes and secret bytes were not printed.

The same owner obtained a fresh native Pasion grant using the restored password pepper and registered a new device. A real Matrix media upload before shutdown was downloaded through the authenticated media API after restoration and had identical bytes, with the original storage path unavailable. Revoking the previous device made its poll fail with 401. Revocation does not immediately shorten the persisted lease TTL: recovery therefore performed explicit owner takeover, advanced the epoch and fenced old execution. A previously running fixture execution became `unknown`, could not start under the new epoch, and was never returned as executable work. Unknown execution evidence and side-effect/usage uncertainty were not cleared.

Explicit reconciliation of the already sent known reply returned the same Matrix transaction, content and event ID. The original Matrix event was fetched after restore and retained the permanent puppet sender and body; the complete original outbox row remained unchanged. A new mention completed poll/ACK/start/durable reply and actual Matrix delivery under the fresh device. Retirement and a late real Matrix join were subsequently reconciled. There was no repeated model inference.

## Persistent paths checked against actual code

| Material | Fixture path | Authority |
| --- | --- | --- |
| AS/HS credentials and registration | `data/agent-appservice.json` | mandatory Appservice initialization |
| Matrix signing key and version | `data/matrix-signing-key.json` | backend `Config::prepare_signing_key` |
| Pasion signing/encryption keys and Matrix shared secret | `data/pasion-secrets.json` | backend `pasion::prepare`, generated `secrets` plus `matrix_secret` |
| Password hashing pepper | `password-pepper` | fixture `[passwords.schemes]` version 1 Argon2id `secret_file`, resolved relative to Pasion config |
| Matrix filesystem media | `media/` | fixture Palpo `[storage]` root, proven with real upload/download |
| Pasion filesystem media | `data/pasion-media/` | backend-managed Pasion storage root; whole data tree copied, no Pasion media API upload asserted |
| Host/Palpo/Pasion config | three root TOML files | same runtime identity, only DB/local root adaptation |

Pasion's default password scheme has no pepper. This exercise explicitly supplies a private pepper file before account bootstrap so successful post-restore password login proves that configured dependency was restored; it does not falsely claim the default generates an extra pepper. Repository binary/static resources and PostgreSQL roles/extensions are existing test prerequisites, not copied private fixture assets.

## Actual boundary

This is a stopped, isolated full three-database plus independent filesystem/key/media restore on the same machine and PostgreSQL cluster. The original live paths are unavailable during recovery. It validates copied private host dependencies, complete relational restoration and application restart, not a production whole-machine disaster recovery rehearsal. PostgreSQL roles/extensions, OS/runtime installation, external object storage, owner-local keyring/crypto state and deployment-specific external secret files remain separate dependencies. The test does not verify host loss, cross-version migration, live cross-database snapshots/PITR, federation continuity, encrypted Rooms or concurrent send failure during the snapshot. A production rehearsal must cover applicable dependencies without changing the homeserver/issuer or running a duplicate AS. A known sent reply is proven here; ambiguous network replies still require stable-transaction recovery, not new inference.
