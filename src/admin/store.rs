use super::*;
use diesel::{
    sql_query,
    sql_types::{BigInt, Jsonb},
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use tokio::sync::Mutex;

#[derive(diesel::QueryableByName)]
struct Document {
    #[diesel(sql_type=Jsonb)]
    body: Value,
}
struct Inner {
    state: Value,
    db: Option<AsyncPgConnection>,
}
/// The admin document and delivery leases commit together. A dedicated advisory
/// lock prevents concurrent processes from racing durable Matrix operation plans.
pub struct Store {
    inner: std::sync::Arc<Mutex<Inner>>,
}
impl Store {
    pub fn memory() -> Self {
        Self {
            inner: std::sync::Arc::new(Mutex::new(Inner {
                state: Self::empty(),
                db: None,
            })),
        }
    }
    fn empty() -> Value {
        json!({"version":1,"fleets":{},"projects":{},"requests":{},"audit":[],"deliveries":[]})
    }
    pub async fn postgres(url: &str) -> anyhow::Result<Self> {
        let mut db = AsyncPgConnection::establish(url).await?;
        #[derive(diesel::QueryableByName)]
        struct Lock {
            #[diesel(sql_type=diesel::sql_types::Bool)]
            locked: bool,
        }
        let lock = sql_query("SELECT pg_try_advisory_lock($1) AS locked")
            .bind::<BigInt, _>(0x484147454e4359_i64)
            .get_result::<Lock>(&mut db)
            .await?;
        anyhow::ensure!(
            lock.locked,
            "another hagency-server owns this admin database"
        );
        // Do not create a schema matching the database role: PostgreSQL
        // searches "$user" before public and would redirect Palpo migrations.
        sql_query("CREATE TABLE IF NOT EXISTS public.hagency_admin_state (id INTEGER PRIMARY KEY CHECK (id=1), body JSONB NOT NULL)").execute(&mut db).await?;
        let row = sql_query("SELECT body FROM public.hagency_admin_state WHERE id=1")
            .load::<Document>(&mut db)
            .await?
            .pop();
        let state = row.map(|r| r.body).unwrap_or_else(Self::empty);
        anyhow::ensure!(state["version"] == 1, "unsupported admin database version");
        let this = Self {
            inner: std::sync::Arc::new(Mutex::new(Inner {
                state,
                db: Some(db),
            })),
        };
        this.change(|st| {
            if st["initializedAt"].is_null() {
                st["initializedAt"] = json!(now());
            }
            Ok(())
        })
        .await?;
        Ok(this)
    }
    pub async fn snapshot(&self) -> Value {
        self.inner.lock().await.state.clone()
    }
    pub(crate) async fn change<T>(&self, f: impl FnOnce(&mut Value) -> Result<T>) -> Result<T> {
        let mut inner = self.inner.clone().lock_owned().await;
        let mut state = inner.state.clone();
        let result = f(&mut state)?;
        if state == inner.state {
            return Ok(result);
        }
        // A request timeout/disconnect must not cancel a database write between
        // commit and updating the in-memory document. The job owns the guard.
        let commit = tokio::spawn(async move {
            if let Some(db) = inner.db.as_mut() {
                sql_query("INSERT INTO public.hagency_admin_state(id,body) VALUES(1,$1) ON CONFLICT(id) DO UPDATE SET body=excluded.body")
                    .bind::<Jsonb,_>(&state).execute(db).await?;
            }
            inner.state = state;
            Ok::<_, ApiError>(())
        });
        commit
            .await
            .map_err(|_| err(503, "storage_unavailable", "Persistence worker failed."))??;
        Ok(result)
    }

    pub(crate) async fn fleet(&self, id: &str) -> Result<Value> {
        let inner = self.inner.lock().await;
        inner.state["fleets"]
            .get(id)
            .cloned()
            .ok_or_else(|| err(404, "not_found", "Fleet not found."))
    }
    pub(crate) async fn save_fleet(&self, id: &str, mut fleet: Value) -> Result<()> {
        self.change(|state| {
            let old = &state["fleets"][id];
            // Relay transactions can arrive while a browser waits for Matrix.
            // Keep their exact probe evidence when saving the operation plan.
            if !fleet["probe"]["challenge"].is_null()
                && fleet["probe"]["challenge"] == old["probe"]["challenge"]
            {
                for name in ["matrixTransactionId", "matrixEventId"] {
                    if !old["probe"][name].is_null() {
                        fleet["probe"][name] = old["probe"][name].clone();
                    }
                }
            }
            state["fleets"][id] = fleet;
            Ok(())
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn invalid_atomic_update_preserves_the_prior_document() {
        let store = Store::memory();
        let before = store.snapshot().await;
        let result: Result<()> = store
            .change(|st| {
                st["fleets"]["bad"] = json!({"token":"never-committed"});
                Err(err(409, "rejected", "invalid update"))
            })
            .await;
        assert!(result.is_err());
        assert_eq!(store.snapshot().await, before);
    }
    #[tokio::test]
    #[ignore = "requires a dedicated HAGENCY_TEST_DATABASE_URL"]
    async fn postgres_restart_retains_queue_and_enforces_single_writer() {
        let url = std::env::var("HAGENCY_TEST_DATABASE_URL").expect("dedicated database URL");
        let store = Store::postgres(&url).await.unwrap();
        let binding = uuid::Uuid::new_v4().to_string();
        store.change(|st| {st["testBinding"]=json!(binding);st["deliveries"]=json!([{"id":"matrix-1","payload":{"events":[]},"token":"lease-1","acked":null}]);Ok(())}).await.unwrap();
        assert!(Store::postgres(&url).await.is_err());
        drop(store);
        let reopened = Store::postgres(&url).await.unwrap();
        let state = reopened.snapshot().await;
        assert_eq!(state["testBinding"], binding);
        assert_eq!(state["deliveries"][0]["token"], "lease-1");
    }
}
