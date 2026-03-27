use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tracing_subscriber::EnvFilter;

/// Initialise tracing and install the global Prometheus metrics recorder.
///
/// Sets up a `tracing-subscriber` with an `EnvFilter` and a `fmt` layer for
/// structured console logging. Installs a Prometheus recorder so that all
/// `metrics::counter!` / `metrics::histogram!` / `metrics::gauge!` calls
/// across the codebase are collected and accessible via the returned handle.
///
/// The returned `PrometheusHandle` must be stored for the lifetime of the
/// process; it is used by the `/metrics` endpoint to render the current
/// snapshot of all recorded metrics.
pub fn init_telemetry() -> PrometheusHandle {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    PrometheusBuilder::new()
        .set_buckets(&[
            0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
        ])
        .expect("failed to set histogram buckets")
        .install_recorder()
        .expect("failed to install Prometheus recorder")
}

/// Spawn a background task that periodically updates DB connection-pool gauges.
///
/// Uses `metrics::gauge!` to record the total and idle connection counts every
/// 15 seconds so that the values appear in the Prometheus scrape endpoint.
pub fn spawn_db_pool_metrics(pool: sqlx::PgPool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
        loop {
            interval.tick().await;
            metrics::gauge!("db_pool_connections").set(pool.size() as f64);
            metrics::gauge!("db_pool_idle_connections").set(pool.num_idle() as f64);
        }
    });
}
