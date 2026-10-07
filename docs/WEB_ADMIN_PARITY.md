# Palpo/Pasion management and the new Agent boundary

[中文](WEB_ADMIN_PARITY.zh-CN.md) · [Documentation](README.md)

The integrated Dioxus console preserves generic Palpo/Pasion administration.
The old Hagency Fleet/Engagement approval product has been replaced, rather than
ported as a compatible second mode. Upstream dependency revisions and copied
frontend provenance remain recorded in Cargo.toml and NOTICE.

| Capability | Current implementation / boundary |
| --- | --- |
| Matrix users/devices, Rooms/Spaces/members/state, media, reports, destinations | Existing generic frontend pages call Palpo's own APIs |
| Appservice listing, registration tokens, server status/actions/notices/notifications | Existing Palpo management pages and API authority checks retained |
| Human registration/password/account status/admin role | Pasion account center/admin APIs and normal provisioning |
| OAuth/individual sessions, upstream providers/links, audit, connectors, notifications | Existing Pasion management pages and Pasion authorization retained |
| Personal Pasion PKCE login, cookie/CSRF bridge, live admin verification and grant handover | Independent `backend/browser_auth.rs`; no old domain store |
| Agent owner, Project=Space, independent Room bindings, creation policy | New PostgreSQL `agent-service/domain`; `/hagency/projects`, `/hagency/agents` |
| Required integrated Appservice, durable AS ACK, owner event queue, fenced execution lease and reply outbox | New `agent-service` and trusted backend Matrix adapters/workers |
| Local Codex/models, Room/user quotas, requester filtering and risky tool decisions | The user's `chrislearn/hagency-client`; no server administrator approval |
| Encrypted Room Agent execution | Deferred pending client crypto; no claim of working encrypted execution |

Removed: Fleet/Hafleet enrollment/pairing/isolated per-install Appservices,
provider resource selection and allocation approvals, Engagement/delegation
import, private account-approval rooms, old request/inbox screens, old miniapp
aliases and native-password UI. Their code, configuration and compatibility
aliases are not part of this release. See [cutover details](../crates/agent-service/legacy-cutover.md).

Pasion owns human identity. Its provisioning synchronizes Matrix records/roles.
The frontend uses member scopes for users and requests extra administrative
scopes only for a verified administrator. Palpo administration checks its local
Matrix role and the host's live Pasion administrative scope. Pasion's own APIs
check their role/scope. Native human-password/account writes are refused at the
existing delegated-auth boundary; Matrix profile/service identity operations
retain their normal APIs.

The first administrator CLI retains Pasion password hashing, bootstrap lock and
normal provisioning. Explicit `--link-existing-matrix-admin` links an existing
active human administrator without changing their Matrix identity; it never
imports Fleet state or overwrites an existing Pasion account.

The new browser BFF is a closed set of Project/Agent/binding operations, not an
arbitrary proxy or a native device executor. Body fields cannot override owner,
Matrix membership or administrator facts. Project policy is governed by real
Space administration rights; server-admin status alone is not an ownership grant.
Padmin's optional separate `palpo_admin` sidecar remains outside this host.

Executed evidence belongs in VALIDATION.md. Backend/frontend compilation and
controlled BrowserAuth checks do not establish full browser visual parity or
successful live Matrix/PKCE transport by themselves.
