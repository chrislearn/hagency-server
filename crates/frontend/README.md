# Hagency frontend

Dioxus 0.7.5 client compiled to WASM and hosted at `/` by `crates/backend`.
The initial source was copied from Padmin (`meldry-com/padmin`, commit
`83d4567470ada808b89914aa0c786cdc3a7ac89a`); see LICENSE and the root NOTICE.

The Matrix/Pasion administration pages are retained. Hagency service pages in
`src/pages/hagency` replace the previous web-admin HTML/JS frontend and call the
same Rust APIs. Login uses native Matrix authentication or Pasion OAuth/PKCE,
according to the host's runtime configuration. Matrix identity and permissions
are verified by the backend; tokens remain in browser memory.

Run `just prepare-frontend` or `just dev` from the workspace root. Generated
assets live under ignored `resources/frontend/public`. See the root README for
component deployment, databases and permissions.
