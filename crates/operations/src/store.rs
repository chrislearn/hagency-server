//! One shared PostgreSQL document for legacy administration, coordinator
//! workflows, audit, notification intents and leased delivery records.
use std::sync::Arc;

use diesel::{
    sql_query,
    sql_types::{BigInt, Bool, Jsonb},
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::{Result, fail};

#[derive(diesel::QueryableByName)]
struct Document {
    #[diesel(sql_type = Jsonb)]
    body: Value,
}
struct Inner {
    state: Value,
    db: Option<AsyncPgConnection>,
}

/// Clones share the same writer and state, including cancellation-safe commits.
/// A database advisory lock excludes another hagency-server process.
/// Historical `fleets`, `fleet` and `fleetId` storage/wire keys are retained;
/// the domain and management UI name is Fleet. Renaming a payload key would
/// invalidate existing content digests, grants and downloaded credentials.
#[derive(Clone)]
pub struct Store {
    inner: Arc<Mutex<Inner>>,
}
impl Store {
    pub fn memory() -> Result<Self> {
        Ok(Self {
            inner: Arc::new(Mutex::new(Inner {
                state: Self::empty(),
                db: None,
            })),
        })
    }
    fn empty() -> Value {
        json!({"version":1,"fleets":{},"projects":{},"requests":{},"audit":[],"deliveries":[]})
    }
    pub async fn postgres(url: &str) -> Result<Self> {
        let mut db = AsyncPgConnection::establish(url)
            .await
            .map_err(|_| fail(503, "workflow_database_unavailable"))?;
        #[derive(diesel::QueryableByName)]
        struct Lock {
            #[diesel(sql_type = Bool)]
            locked: bool,
        }
        let lock = sql_query("SELECT pg_try_advisory_lock($1) AS locked")
            .bind::<BigInt, _>(0x484147454e4359_i64)
            .get_result::<Lock>(&mut db)
            .await?;
        if !lock.locked {
            return Err(fail(409, "workflow_database_in_use"));
        }
        sql_query("CREATE TABLE IF NOT EXISTS public.hagency_admin_state (id INTEGER PRIMARY KEY CHECK (id=1), body JSONB NOT NULL)").execute(&mut db).await?;
        let row = sql_query("SELECT body FROM public.hagency_admin_state WHERE id=1")
            .load::<Document>(&mut db)
            .await?
            .pop();
        let state = row.map(|r| r.body).unwrap_or_else(Self::empty);
        if !state.is_object() || state["version"] != 1 {
            return Err(fail(503, "unsupported_workflow_database_version"));
        }
        // Preserve every existing field. A missing delivery list is initialized
        // only for an empty/older document; malformed data fails closed.
        let this = Self {
            inner: Arc::new(Mutex::new(Inner {
                state,
                db: Some(db),
            })),
        };
        this.transaction(|st| {
            if st["deliveries"].is_null() {
                st["deliveries"] = json!([]);
            }
            if !st["deliveries"].is_array() {
                return Err(fail(503, "workflow_state_invalid"));
            }
            Ok(())
        })
        .await?;
        Ok(this)
    }
    pub async fn read(&self) -> Result<Value> {
        Ok(self.inner.lock().await.state.clone())
    }
    /// No external I/O inside the closure. A verdict, audit record, outbox and
    /// delivery lease either commit together or leave the previous state intact.
    pub async fn transaction<T>(
        &self,
        operation: impl FnOnce(&mut Value) -> Result<T>,
    ) -> Result<T> {
        let mut inner = self.inner.clone().lock_owned().await;
        let mut state = inner.state.clone();
        let result = operation(&mut state)?;
        if state == inner.state {
            return Ok(result);
        }
        // Own the writer through commit even if the HTTP future is cancelled.
        let commit = tokio::spawn(async move {
            if let Some(db) = inner.db.as_mut() {
                sql_query("INSERT INTO public.hagency_admin_state(id,body) VALUES(1,$1) ON CONFLICT(id) DO UPDATE SET body=excluded.body")
                    .bind::<Jsonb, _>(&state).execute(db).await?;
            }
            inner.state = state;
            Ok::<_, crate::Error>(())
        });
        commit
            .await
            .map_err(|_| fail(503, "workflow_commit_failed"))??;
        Ok(result)
    }
    pub async fn bind(&self, server: &str, origin: &str) -> Result<()> {
        self.transaction(|state| {
            let binding = json!({"serverName":server,"palpoOrigin":origin});
            if !state["serverBinding"].is_null() && state["serverBinding"] != binding {
                return Err(fail(409, "workflow_server_binding_mismatch"));
            }
            state["serverBinding"] = binding;
            Ok(())
        })
        .await
    }
}
