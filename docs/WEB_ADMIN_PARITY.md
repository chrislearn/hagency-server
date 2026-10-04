# Palpo web-admin migration and unified accounts

Source baseline: local Palpo `web-admin` at frontend redesign commit
`1032153e` (the embedded Palpo revision is recorded in Cargo.toml).
The goal is equivalent Hagency workflows in Padmin's Dioxus UI, not identical
HTML, DOM selectors or a Node service. Account authentication intentionally
changes to Pasion in the default deployment.

| Workflow | Rust/Dioxus implementation | Verification |
| --- | --- | --- |
| Provider grant, isolated App Service, resumable install, drift detection | backend admin/fleet.rs; frontend Hagency connections | HTTP contract; live Matrix integration |
| Outbound migration/rotation, queue limits, leases, ACK, online/current-generation proof | backend admin/outbound.rs; connections and capacity inspection | HTTP contract; live Matrix integration |
| Owner-only pairing/config download, exact event receipt and reception memberships | backend admin/workflow.rs; My Hagencys | HTTP contract; live Matrix integration |
| Project creation or existing room binding, encrypted private approval room, owner/member authority | backend admin/workflow.rs; Projects | HTTP contract; live Matrix integration |
| Published resource and role selection, named Agent, token/daily quota, durable request/retry | backend admin/workflow.rs; Request an agent | HTTP contract; frontend role/resource checks |
| Provider decisions, actual running configuration/preparation, stale status, actual room admission before usable | backend admin/workflow.rs; Agent requests | HTTP contract; live Matrix integration; readiness grouping regression |
| Managed identities, restricted rename, retirement with Matrix deactivation/membership checks | backend admin/fleet.rs; Agent identities | HTTP contract |
| Audit records, owner isolation, credential redaction, Origin/CSRF/live role enforcement | backend admin/api.rs | HTTP contract |
| Callback proof renewal with one-minute retry; failures shown and submission blocked | Hagency common.rs and page warnings | source comparison; compile; browser inspection |
| Native account requests/receipts and private Matrix administrator-room approval | backend admin/accounts.rs; Hagency approvals | preserved for explicit native-auth mode; HTTP contract |

The migrated request screen now shows actual `provider.serving` framework,
model, reasoning and tier, fulfillment phase/error, delivery/review explanations,
stale status and the original attention/review/usable grouping. An active but
unusable agent remains outside Ready to use. Resource cards can populate the
Agent definition. Callback policy is visible and an unconfigured callback
choice is disabled. Renewal errors remain visible during the retry interval.

## Intentional identity changes

Default `[hagency].delegate_matrix_auth = true` means Pasion owns account
registration, passwords, account status and administrator role. Its existing
provisioning jobs synchronize Matrix identities/roles. The integrated console
has one Pasion login entry; members get client/device scopes, administrators
continue authorization for their verified account with admin scopes.

The preliminary member authorization stays only in the host's HttpOnly session
until its replacement has been verified. The host then revokes that preliminary
OAuth grant directly in Pasion, preserving the current Matrix device. This
avoids a second password prompt and leaves no preliminary active grant after
successful handover. Bearer/refresh tokens remain memory-only in the frontend.

Palpo's embedded admin middleware checks its local Matrix role. The host adds
live Pasion introspection with the exact Palpo admin scope. Native account
creation/password writes are refused at this boundary; Matrix profile updates
and App Service identities remain available. Pasion's own APIs enforce their
admin role/scope. Revocations are checked on the next request without a cached
introspection grace period. Human creation/reset/role changes go through
Pasion Account management; CSV native-user import is hidden in this mode.

The old native Matrix signup/Robrix approval form is not a second registration
system in a unified deployment: `/account-request` redirects to Pasion signup,
and its legacy approval menu is hidden. Its optional business workflow is
preserved in compatibility mode, not reimplemented as Pasion approval policy.
A deployment requiring that exact pre-registration approval policy still needs
a Pasion policy/workflow integration before replacing its legacy signup flow.

The first administrator CLI creates a Pasion account, hashes through Pasion's
password manager, holds Pasion's bootstrap lock, and queues normal provisioning.
An explicit `--link-existing-matrix-admin` option preserves an existing active
human administrator's Matrix ID. Existing Pasion accounts/administrators are
never overwritten by bootstrap.

Padmin's separate optional `palpo_admin` operation sidecar is not part of Palpo
`web-admin` and remains outside this migration; its menus remain disabled.
