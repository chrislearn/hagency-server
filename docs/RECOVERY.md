# Backup and recovery for the owner architecture

[中文](RECOVERY.zh-CN.md)

This procedure applies only to the new Hagency structures. It does not import Fleet data or change an Agent's permanent owner.

Back up the entire separate Hagency PostgreSQL database together with deployment configuration and private Appservice registration credentials. Preserve owner/puppet identities, issuer/subject/MXID, command digests, binding generations, lease epochs, AS transaction digests and stable reply transaction IDs. Partial table exports cannot restore reliable execution.

Palpo and Pasion retain their own backup procedures. Record all three recovery points; Hagency recovery must not reset their databases. If restored Matrix history no longer contains an event recorded as sent, reconcile the original event/transaction/payload in isolation. Do not regenerate the model output.

For a client backup, stop the owner host and its user service before copying the complete private owner state directory, including ledger, Room files and Codex context. Copying only the SQLite main file while omitting its WAL is unsafe; use a stopped directory copy or SQLite's backup API. Provider keyring login is separate and requires the original user to sign in again. Pasion authorization also requires fresh login.

Recovery sequence:

1. Record deployment identity, issuer, homeserver, code and schema versions. Pause Agent service and stop Hagency workers before the snapshot.
2. Use PostgreSQL `pg_dump --format=custom` on the complete isolated Hagency database. Supply secrets through the normal database credential mechanism, never command arguments or reports. Encrypt the private AS credential backup separately.
3. Restore into a new isolated database/instance with the same identity. Do not overwrite production or register the restored AS against a different real homeserver. Do not run two outward-facing instances sharing one deployment identity.
4. Verify immutable owner/puppet identity, binding generation, original dispatch/execution, reply digest/transaction and AS digest. Incompatible schemas must fail; do not edit version markers or add compatibility migration.
5. Revoke the old device and explicitly take over its lease, or wait for actual lease expiry. Revocation alone does not shorten a recorded lease TTL. The original owner logs in through Pasion and registers a new device. Lost running authority becomes unknown; do not requeue it or release usage holds.
6. Reconcile only a known durable result through `execution/replies/reconcile-known`, using the original execution, content and transaction under fresh authority. Keep sent event IDs; ambiguous sends reuse the original transaction. Irreversible delivery blocks stay blocked.
7. Recheck live Space/Room membership and pauses before explicit owner resume. Verify the startup AS roundtrip separately from provider login and task completion. `/readyz` proves the startup roundtrip, not continuous health.

An offline full dump/restore of all three databases passed in an isolated same-host fixture, including original deployment restart, fresh login/device takeover, unknown execution preservation, sent reply identity preservation and a fresh message roundtrip. See the [exercise evidence](BACKUP_RESTORE_VALIDATION.md). Independent-filesystem recovery also passed: the original directory was moved out of reach, and new private files/three databases preserved AS/Matrix/Pasion keys, configured pepper and real Matrix media download. Whole-host/cross-host recovery, external object storage, Pasion media API and release-environment disaster recovery remain separate checks.

Routed AS plaintext is compacted only after routing completes. Permanent transaction ID/digest tombstones preserve replay detection and continue consuming storage; pending work remains capacity bounded. Unknown execution, unresolved usage, unsent replies and tool side-effect evidence must not be deleted as ordinary logs. Recovery never changes ownership.
