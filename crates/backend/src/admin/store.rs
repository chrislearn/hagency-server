use super::*;

/// Legacy routes and Operations share one PostgreSQL writer and document.
pub struct Store {
    pub(crate) shared: hagency_operations::store::Store,
}
impl Store {
    pub fn memory() -> Self {
        Self {
            shared: hagency_operations::store::Store::memory().expect("memory store"),
        }
    }
    pub async fn postgres(url: &str) -> anyhow::Result<Self> {
        let shared = hagency_operations::store::Store::postgres(url).await?;
        let this = Self { shared };
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
        self.shared.read().await.expect("validated shared document")
    }
    pub(crate) async fn change<T>(&self, f: impl FnOnce(&mut Value) -> Result<T>) -> Result<T> {
        self.shared
            .transaction(|st| {
                f(st).map_err(|e| hagency_operations::Error {
                    status: e.status,
                    code: e.code,
                    message: e.message,
                })
            })
            .await
            .map_err(ApiError::from)
    }
    pub(crate) async fn fleet(&self, id: &str) -> Result<Value> {
        let state = self.snapshot().await;
        state["fleets"]
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
