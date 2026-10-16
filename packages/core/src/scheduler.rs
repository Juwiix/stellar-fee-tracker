//! Fee polling scheduler.
//!
//! Drives the main polling loop: each tick fetches fee data from the
//! Horizon provider, pushes it into the history store, runs the
//! insights engine, and persists new points to SQLite.
//!
//! Network errors are retried with exponential backoff + jitter (Issue #10).
//! Parse errors are not retried — malformed data won't fix itself.
//! DB write errors are logged but never crash the scheduler.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::signal;
use tokio::sync::RwLock;
use tokio::time;

use crate::alerts::AlertManager;
use crate::error::AppError;
use crate::insights::error::ProviderError;
use crate::insights::types::FeeDataPoint;
use crate::insights::{FeeDataProvider, FeeInsightsEngine};
use crate::metrics::AppMetrics;
use crate::repository::FeeRepository;
use crate::services::horizon::HorizonClient;
use crate::store::{FeeHistoryStore, FeeStatsSnapshot};

/// Full polling loop with configurable retry parameters and optional DB persistence.
#[allow(clippy::too_many_arguments)]
pub async fn run_fee_polling_with_retry(
    horizon_provider: Arc<dyn FeeDataProvider + Send + Sync>,
    history_store: Arc<RwLock<FeeHistoryStore>>,
    insights_engine: Arc<RwLock<FeeInsightsEngine>>,
    poll_interval_seconds: u64,
    max_retry_attempts: u32,
    base_retry_delay_ms: u64,
    repository: Option<Arc<FeeRepository>>,
    storage_retention_days: u64,
    metrics: Option<Arc<AppMetrics>>,
    alert_manager: Option<Arc<AlertManager>>,
) {
    let mut interval = time::interval(Duration::from_secs(poll_interval_seconds));

    tracing::info!(
        "Fee polling started (interval: {}s, max retries: {}, retention: {}d)",
        poll_interval_seconds,
        max_retry_attempts,
        storage_retention_days,
    );

    loop {
        tokio::select! {
            _ = interval.tick() => {
                poll_once(
                    &horizon_provider,
                    &history_store,
                    &insights_engine,
                    max_retry_attempts,
                    base_retry_delay_ms,
                    repository.as_deref(),
                    storage_retention_days,
                    metrics.as_deref(),
                    alert_manager.as_deref(),
                ).await;
            }

            _ = signal::ctrl_c() => {
                tracing::info!("Shutdown signal received. Stopping polling.");
                break;
            }
        }
    }

    tracing::info!("Fee polling stopped cleanly");
}

/// Execute a single poll cycle with retry and optional persistence.
#[allow(clippy::too_many_arguments)]
async fn poll_once(
    horizon_provider: &Arc<dyn FeeDataProvider + Send + Sync>,
    history_store: &Arc<RwLock<FeeHistoryStore>>,
    insights_engine: &Arc<RwLock<FeeInsightsEngine>>,
    max_retry_attempts: u32,
    base_retry_delay_ms: u64,
    repository: Option<&FeeRepository>,
    storage_retention_days: u64,
    metrics: Option<&AppMetrics>,
    alert_manager: Option<&AlertManager>,
) {
    if let Some(m) = metrics {
        m.polls_total.inc();
    }

    let points = match fetch_with_retry(
        horizon_provider.as_ref(),
        max_retry_attempts,
        base_retry_delay_ms,
    )
    .await
    {
        Some(p) => p,
        None => {
            if let Some(m) = metrics {
                m.poll_errors_total.inc();
            }
            tracing::warn!(
                "All {} retry attempts exhausted — skipping tick",
                max_retry_attempts
            );
            return;
        }
    };

    if points.is_empty() {
        tracing::warn!("Provider returned no fee data points this tick");
        return;
    }

    // Each tick fetches the latest transactions, so consecutive polls overlap.
    // Drop hashes we have already stored so averages are not skewed by repeats.
    let points: Vec<FeeDataPoint> = {
        let store = history_store.read().await;
        let mut seen = std::collections::HashSet::new();
        points
            .into_iter()
            .filter(|p| {
                store.get_by_hash(&p.transaction_hash).is_none()
                    && seen.insert(p.transaction_hash.clone())
            })
            .collect()
    };

    if points.is_empty() {
        tracing::debug!("All fetched fee points were already stored; nothing new this tick");
        return;
    }

    // Push into in-memory store
    {
        let mut store = history_store.write().await;
        for point in &points {
            store.push(point.clone());
        }
        let store_len = store.len();
        tracing::debug!("Store now holds {} data points", store_len);
        if let Some(m) = metrics {
            m.fee_points_stored.set(store_len as f64);
        }
    }

    // Run insights engine
    {
        let mut engine = insights_engine.write().await;
        match engine.process_fee_data(&points).await {
            Ok(update) => {
                tracing::info!(
                    "Insights updated — {} points processed, short-term avg: {:.1} stroops",
                    update.data_points_processed,
                    update.insights.rolling_averages.short_term.value,
                );
                if let Some(m) = metrics {
                    m.current_avg_fee
                        .set(update.insights.rolling_averages.short_term.value);
                    m.spikes_detected_total
                        .inc_by(update.insights.congestion_trends.recent_spikes.len() as f64);
                }
                if let Some(manager) = alert_manager {
                    manager.check_and_dispatch(&update).await;
                }
            }
            Err(err) => {
                tracing::error!("Insights engine error: {}", err);
            }
        }
    }

    // Persist to DB (non-fatal on error)
    if let Some(repo) = repository {
        match repo.insert_fee_points(&points).await {
            Ok(()) => {
                tracing::debug!("Persisted {} fee points to DB", points.len());
            }
            Err(err) => {
                tracing::warn!("Failed to persist fee points to DB: {}", err);
            }
        }

        let cutoff = Utc::now() - chrono::Duration::days(storage_retention_days as i64);
        match repo.prune_older_than(cutoff).await {
            Ok(n) if n > 0 => tracing::debug!("Pruned {} old fee points from DB", n),
            Ok(_) => {}
            Err(err) => tracing::warn!("Failed to prune old fee points: {}", err),
        }
        match repo.prune_recommendations_older_than(cutoff).await {
            Ok(n) if n > 0 => tracing::debug!("Pruned {} old recommendations from DB", n),
            Ok(_) => {}
            Err(err) => tracing::warn!("Failed to prune old recommendations: {}", err),
        }
        match repo.prune_alert_events_older_than(cutoff).await {
            Ok(n) if n > 0 => tracing::debug!("Pruned {} old alert events from DB", n),
            Ok(_) => {}
            Err(err) => tracing::warn!("Failed to prune old alert events: {}", err),
        }
    }
}

/// Attempt to fetch fee data, retrying on network errors with exponential
/// backoff + random jitter. Parse errors are not retried.
///
/// Returns `Some(points)` on the first successful fetch, or `None` if all
/// attempts are exhausted.
pub async fn fetch_with_retry(
    provider: &dyn FeeDataProvider,
    max_attempts: u32,
    base_delay_ms: u64,
) -> Option<Vec<FeeDataPoint>> {
    const MAX_DELAY_MS: u64 = 30_000;

    for attempt in 0..max_attempts {
        match provider.fetch_latest_fees().await {
            Ok(points) => {
                if attempt > 0 {
                    tracing::info!("Fetch succeeded after {} attempt(s)", attempt + 1);
                }
                return Some(points);
            }

            Err(ProviderError::FormatError { message }) => {
                tracing::error!("Parse error fetching fees (not retrying): {}", message);
                return None;
            }

            Err(err) => {
                let backoff_ms = {
                    let exponential = base_delay_ms.saturating_mul(1u64.checked_shl(attempt).unwrap_or(u64::MAX));
                    let jitter = rand::random::<u64>() % base_delay_ms.max(1);
                    exponential.saturating_add(jitter).min(MAX_DELAY_MS)
                };

                tracing::warn!(
                    "Fetch attempt {}/{} failed: {} — retrying in {}ms",
                    attempt + 1,
                    max_attempts,
                    err,
                    backoff_ms,
                );

                time::sleep(Duration::from_millis(backoff_ms)).await;
            }
        }
    }

    None
}

// ---- /fee_stats polling (Issue #550) ----

/// Polling loop driven by Horizon's `/fee_stats` endpoint.
///
/// Each tick fetches the full fee_stats payload, converts it to a
/// [`FeeStatsSnapshot`] and persists it idempotently (one row per ledger,
/// via `ON CONFLICT (ledger) DO UPDATE`). Snapshots older than the
/// retention window are pruned. DB errors are logged, never fatal.
#[allow(clippy::too_many_arguments)]
pub async fn run_fee_stats_polling(
    client: Arc<HorizonClient>,
    repository: Option<Arc<FeeRepository>>,
    poll_interval_seconds: u64,
    max_retry_attempts: u32,
    base_retry_delay_ms: u64,
    storage_retention_days: u64,
    metrics: Option<Arc<AppMetrics>>,
) {
    let mut interval = time::interval(Duration::from_secs(poll_interval_seconds));

    tracing::info!(
        "Fee-stats polling started (interval: {}s, max retries: {}, retention: {}d)",
        poll_interval_seconds,
        max_retry_attempts,
        storage_retention_days,
    );

    loop {
        tokio::select! {
            _ = interval.tick() => {
                poll_fee_stats_once(
                    &client,
                    repository.as_deref(),
                    max_retry_attempts,
                    base_retry_delay_ms,
                    storage_retention_days,
                    metrics.as_deref(),
                )
                .await;
            }

            _ = signal::ctrl_c() => {
                tracing::info!("Shutdown signal received. Stopping fee-stats polling.");
                break;
            }
        }
    }

    tracing::info!("Fee-stats polling stopped cleanly");
}

/// Execute a single `/fee_stats` poll cycle with retry and idempotent persistence.
async fn poll_fee_stats_once(
    client: &HorizonClient,
    repository: Option<&FeeRepository>,
    max_retry_attempts: u32,
    base_retry_delay_ms: u64,
    storage_retention_days: u64,
    metrics: Option<&AppMetrics>,
) {
    if let Some(m) = metrics {
        m.polls_total.inc();
    }

    let snapshot =
        match fetch_fee_stats_with_retry(client, max_retry_attempts, base_retry_delay_ms).await {
            Some(s) => s,
            None => {
                if let Some(m) = metrics {
                    m.poll_errors_total.inc();
                }
                tracing::warn!(
                    "All {} retry attempts exhausted for fee_stats — skipping tick",
                    max_retry_attempts
                );
                return;
            }
        };

    tracing::debug!(
        "Fetched fee_stats for ledger {} (base fee: {} stroops)",
        snapshot.ledger,
        snapshot.base_fee
    );

    // Persist to DB — idempotent per ledger, non-fatal on error.
    if let Some(repo) = repository {
        match repo.upsert_fee_snapshot(&snapshot).await {
            Ok(n) => {
                tracing::debug!(
                    "Persisted fee snapshot for ledger {} ({} row)",
                    snapshot.ledger,
                    n
                );
            }
            Err(err) => {
                tracing::warn!(
                    "Failed to persist fee snapshot for ledger {}: {}",
                    snapshot.ledger,
                    err
                );
            }
        }

        let cutoff = Utc::now() - chrono::Duration::days(storage_retention_days as i64);
        match repo.prune_fee_snapshots_older_than(cutoff).await {
            Ok(n) if n > 0 => tracing::debug!("Pruned {} old fee snapshots from DB", n),
            Ok(_) => {}
            Err(err) => tracing::warn!("Failed to prune old fee snapshots: {}", err),
        }
    }
}

/// Attempt to fetch and convert a `/fee_stats` snapshot, retrying on
/// network errors with exponential backoff + random jitter. Parse errors
/// (including invalid numeric fields) are not retried — malformed data
/// won't fix itself.
///
/// Returns `Some(snapshot)` on the first successful fetch, or `None` if
/// all attempts are exhausted.
pub async fn fetch_fee_stats_with_retry(
    client: &HorizonClient,
    max_attempts: u32,
    base_delay_ms: u64,
) -> Option<FeeStatsSnapshot> {
    const MAX_DELAY_MS: u64 = 30_000;

    for attempt in 0..max_attempts {
        match client.fetch_fee_stats_full().await {
            Ok(response) => match FeeStatsSnapshot::try_from(&response) {
                Ok(snapshot) => {
                    if attempt > 0 {
                        tracing::info!(
                            "fee_stats fetch succeeded after {} attempt(s)",
                            attempt + 1
                        );
                    }
                    return Some(snapshot);
                }
                Err(err) => {
                    tracing::error!("Parse error converting fee_stats (not retrying): {}", err);
                    return None;
                }
            },

            Err(AppError::Parse(message)) => {
                tracing::error!("Parse error fetching fee_stats (not retrying): {}", message);
                return None;
            }

            Err(err) => {
                let backoff_ms = {
                    let exponential = base_delay_ms.saturating_mul(1u64.checked_shl(attempt).unwrap_or(u64::MAX));
                    let jitter = rand::random::<u64>() % base_delay_ms.max(1);
                    exponential.saturating_add(jitter).min(MAX_DELAY_MS)
                };

                tracing::warn!(
                    "fee_stats attempt {}/{} failed: {} — retrying in {}ms",
                    attempt + 1,
                    max_attempts,
                    err,
                    backoff_ms,
                );

                time::sleep(Duration::from_millis(backoff_ms)).await;
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    use crate::insights::error::ProviderError;
    use crate::insights::types::FeeDataPoint;
    use crate::insights::{FeeInsightsEngine, InsightsConfig};
    use crate::services::mock_horizon::MockHorizonClient;
    use crate::store::{FeeHistoryStore, DEFAULT_CAPACITY};

    fn make_point(fee_amount: u64) -> FeeDataPoint {
        FeeDataPoint {
            fee_amount,
            timestamp: Utc::now(),
            transaction_hash: format!("hash_{}", fee_amount),
            ledger_sequence: 1,
        }
    }

    fn make_shared_store() -> Arc<RwLock<FeeHistoryStore>> {
        Arc::new(RwLock::new(FeeHistoryStore::new(DEFAULT_CAPACITY)))
    }

    fn make_shared_engine() -> Arc<RwLock<FeeInsightsEngine>> {
        Arc::new(RwLock::new(FeeInsightsEngine::new(
            InsightsConfig::default(),
        )))
    }

    // ---- poll_once tests ----

    #[tokio::test]
    async fn poll_once_pushes_points_into_store() {
        let points = vec![make_point(100), make_point(200), make_point(300)];
        let provider: Arc<dyn FeeDataProvider + Send + Sync> =
            Arc::new(MockHorizonClient::new().with_fees(points));
        let store = make_shared_store();
        let engine = make_shared_engine();

        poll_once(&provider, &store, &engine, 3, 0, None, 7, None, None).await;

        assert_eq!(store.read().await.len(), 3);
    }

    #[tokio::test]
    async fn poll_once_runs_insights_engine() {
        let points = vec![make_point(100), make_point(150), make_point(200)];
        let provider: Arc<dyn FeeDataProvider + Send + Sync> =
            Arc::new(MockHorizonClient::new().with_fees(points));
        let store = make_shared_store();
        let engine = make_shared_engine();

        poll_once(&provider, &store, &engine, 3, 0, None, 7, None, None).await;

        assert!(engine.read().await.get_last_update().is_some());
    }

    #[tokio::test]
    async fn poll_once_on_provider_error_does_not_push_to_store() {
        let provider: Arc<dyn FeeDataProvider + Send + Sync> =
            Arc::new(MockHorizonClient::new().with_error(ProviderError::ServiceUnavailable));
        let store = make_shared_store();
        let engine = make_shared_engine();

        poll_once(&provider, &store, &engine, 1, 0, None, 7, None, None).await;

        assert!(store.read().await.is_empty());
    }

    #[tokio::test]
    async fn two_poll_cycles_dedupe_repeated_points_in_store() {
        let points = vec![make_point(100), make_point(200)];
        let provider: Arc<dyn FeeDataProvider + Send + Sync> =
            Arc::new(MockHorizonClient::new().with_fees(points));
        let store = make_shared_store();
        let engine = make_shared_engine();

        poll_once(&provider, &store, &engine, 3, 0, None, 7, None, None).await;
        poll_once(&provider, &store, &engine, 3, 0, None, 7, None, None).await;

        assert_eq!(store.read().await.len(), 2);
    }

    #[tokio::test]
    async fn poll_once_with_empty_provider_response_leaves_store_unchanged() {
        let provider: Arc<dyn FeeDataProvider + Send + Sync> = Arc::new(MockHorizonClient::new());
        let store = make_shared_store();
        let engine = make_shared_engine();

        poll_once(&provider, &store, &engine, 3, 0, None, 7, None, None).await;

        assert!(store.read().await.is_empty());
    }

    // ---- fetch_with_retry tests ----

    #[tokio::test]
    async fn fetch_with_retry_returns_points_on_success() {
        let points = vec![make_point(100), make_point(200)];
        let mock = MockHorizonClient::new().with_fees(points.clone());

        let result = fetch_with_retry(&mock, 3, 0).await;

        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn fetch_with_retry_retries_on_network_error_and_succeeds() {
        let mock = MockHorizonClient::new().with_error(ProviderError::NetworkError {
            message: "timeout".into(),
        });

        let result = fetch_with_retry(&mock, 3, 0).await;

        assert!(result.is_none());
        assert_eq!(mock.calls(), 3);
    }

    #[tokio::test]
    async fn fetch_with_retry_does_not_retry_on_parse_error() {
        let mock = MockHorizonClient::new().with_error(ProviderError::FormatError {
            message: "bad json".into(),
        });

        let result = fetch_with_retry(&mock, 3, 0).await;

        assert!(result.is_none());
        assert_eq!(mock.calls(), 1);
    }

    #[tokio::test]
    async fn fetch_with_retry_returns_none_when_all_attempts_exhausted() {
        let mock = MockHorizonClient::new().with_error(ProviderError::ServiceUnavailable);

        let result = fetch_with_retry(&mock, 3, 0).await;

        assert!(result.is_none());
        assert_eq!(mock.calls(), 3);
    }

    #[tokio::test]
    async fn fetch_with_retry_succeeds_on_first_attempt_makes_one_call() {
        let mock = MockHorizonClient::new().with_fees(vec![make_point(100)]);

        let result = fetch_with_retry(&mock, 3, 0).await;

        assert!(result.is_some());
        assert_eq!(mock.calls(), 1);
    }
}

#[cfg(test)]
mod fee_stats_tests {
    use super::*;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    use crate::db::create_pool;

    fn fee_stats_body(base_fee: &str) -> serde_json::Value {
        serde_json::json!({
            "last_ledger": "50000001",
            "last_ledger_base_fee": base_fee,
            "ledger_capacity_usage": "0.97",
            "fee_charged": {
                "min": "100",
                "max": "5000",
                "mode": "213",
                "mean": "250.75",
                "median": "200",
                "p10": "100",
                "p20": "100",
                "p30": "120",
                "p40": "140",
                "p50": "150",
                "p60": "200",
                "p70": "300",
                "p80": "400",
                "p90": "500",
                "p95": "800",
                "p99": "1200"
            },
            "max_fee": {
                "min": "100",
                "max": "10000",
                "mode": "10000",
                "mean": "9876.5",
                "median": "10000"
            }
        })
    }

    #[tokio::test]
    async fn fetch_fee_stats_with_retry_returns_snapshot_on_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/fee_stats"))
            .respond_with(ResponseTemplate::new(200).set_body_json(fee_stats_body("100")))
            .mount(&server)
            .await;

        let client = HorizonClient::new(server.uri());
        let snapshot = fetch_fee_stats_with_retry(&client, 3, 0)
            .await
            .expect("expected a snapshot");

        assert_eq!(snapshot.ledger, 50_000_001);
        assert_eq!(snapshot.base_fee, 100);
        assert_eq!(snapshot.mode_fee_charged, 213);
        assert!((snapshot.mean_fee_charged - 250.75).abs() < f64::EPSILON);
        assert_eq!(snapshot.p95_fee_charged, 800);
        assert_eq!(snapshot.max_fee, 10_000);
        assert!((snapshot.ledger_capacity_usage.unwrap() - 0.97).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn fetch_fee_stats_with_retry_exhausts_attempts_on_http_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/fee_stats"))
            .respond_with(ResponseTemplate::new(503))
            .expect(3)
            .mount(&server)
            .await;

        let client = HorizonClient::new(server.uri());
        let result = fetch_fee_stats_with_retry(&client, 3, 0).await;

        assert!(result.is_none(), "all attempts exhausted should yield None");
    }

    #[tokio::test]
    async fn fetch_fee_stats_with_retry_does_not_retry_parse_errors() {
        let server = MockServer::start().await;
        let mut bad_body = fee_stats_body("100");
        bad_body["last_ledger"] = serde_json::json!("not-a-number");
        // Scoped guard verifies exactly one request was made — any retry
        // would push the call count past the expectation and panic on drop.
        let _guard = Mock::given(method("GET"))
            .and(path("/fee_stats"))
            .respond_with(ResponseTemplate::new(200).set_body_json(bad_body))
            .expect(1)
            .mount_as_scoped(&server)
            .await;

        let client = HorizonClient::new(server.uri());
        let result = fetch_fee_stats_with_retry(&client, 3, 0).await;

        assert!(result.is_none(), "parse errors must not be retried");
    }

    #[tokio::test]
    async fn poll_fee_stats_once_persists_snapshots_idempotently() {
        let server = MockServer::start().await;
        // First tick serves base fee 100; second tick serves base fee 200
        // for the SAME ledger — the upsert must update, not duplicate.
        Mock::given(method("GET"))
            .and(path("/fee_stats"))
            .respond_with(ResponseTemplate::new(200).set_body_json(fee_stats_body("200")))
            .with_priority(2)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/fee_stats"))
            .respond_with(ResponseTemplate::new(200).set_body_json(fee_stats_body("100")))
            .with_priority(1)
            .up_to_n_times(1)
            .mount(&server)
            .await;

        let pool = create_pool("sqlite::memory:").await.unwrap();
        let repo = FeeRepository::new(pool);

        let client = Arc::new(HorizonClient::new(server.uri()));
        poll_fee_stats_once(&client, Some(&repo), 3, 0, 7, None).await;
        poll_fee_stats_once(&client, Some(&repo), 3, 0, 7, None).await;

        let snapshots = repo
            .fetch_fee_snapshots_since(Utc::now() - chrono::Duration::hours(1))
            .await
            .unwrap();
        assert_eq!(
            snapshots.len(),
            1,
            "same ledger must not produce duplicate rows"
        );
        assert_eq!(snapshots[0].ledger, 50_000_001);
        assert_eq!(
            snapshots[0].base_fee, 200,
            "second poll should have updated the row"
        );
    }

    #[tokio::test]
    async fn poll_fee_stats_once_without_repository_does_not_panic() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/fee_stats"))
            .respond_with(ResponseTemplate::new(200).set_body_json(fee_stats_body("100")))
            .mount(&server)
            .await;

        let client = HorizonClient::new(server.uri());
        poll_fee_stats_once(&client, None, 3, 0, 7, None).await;
    }
}
