use std::cell::RefCell;

/// A single API call metric.
#[derive(Clone, Debug)]
pub struct ApiMetric {
    pub url: String,
    pub method: String,
    pub duration_ms: f64,
    pub status: u16,
    pub timestamp: f64,
}

thread_local! {
    static API_METRICS: RefCell<Vec<ApiMetric>> = RefCell::new(Vec::new());
}

/// Record an API call metric.
pub fn record_api_call(url: &str, method: &str, duration_ms: f64, status: u16) {
    let metric = ApiMetric {
        url: url.to_string(),
        method: method.to_string(),
        duration_ms,
        status,
        timestamp: js_sys::Date::now(),
    };
    API_METRICS.with(|m| {
        let mut metrics = m.borrow_mut();
        metrics.push(metric);
        // Keep at most 200 entries to avoid unbounded growth
        if metrics.len() > 200 {
            let drain_count = metrics.len() - 200;
            metrics.drain(..drain_count);
        }
    });
}

/// Return the average latency (in ms) across all recorded metrics, or 0.0 if none.
pub fn average_latency() -> f64 {
    API_METRICS.with(|m| {
        let metrics = m.borrow();
        if metrics.is_empty() {
            return 0.0;
        }
        let total: f64 = metrics.iter().map(|m| m.duration_ms).sum();
        total / metrics.len() as f64
    })
}
