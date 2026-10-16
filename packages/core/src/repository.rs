//! Database repository for fee data persistence.
//!
//! All SQLite read/write logic lives here. The scheduler calls
//! [`FeeRepository::insert_fee_points`] after each poll tick and
//! [`FeeRepository::prune_older_than`] to keep the database bounded.
//!
//! On startup, [`FeeRepository::fetch_since`] rehydrates the in-memory
//! [`FeeHistoryStore`] from the last 24 hours of persisted data.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::insights::types::FeeDataPoint;
use crate::store::FeeStatsSnapshot;

/// Valid threshold values for alert configurations.
/// Must match the `SpikeSeverity` enum variants used by the insights engine.
pub const VALID_THRESHOLDS: &[&str] = &["Minor", "Moderate", "Major", "Critical"];

/// Valid alert type values. `spike` is the original/default behavior;
/// the rest are added for Advisor-relevant conditions (Issue #556).
pub const VALID_ALERT_TYPES: &[&str] = &["spike", "recovery", "good_window", "stale_data"];

/// A single alert webhook configuration row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    pub id: i64,
    pub webhook_url: String,
    pub threshold: String,
    /// One of `spike | recovery | good_window | stale_data`. Defaults to
    /// `spike` for rows created before Issue #556.
    pub alert_type: String,
    pub enabled: bool,
    pub created_at: String,
}

/// A single fired-alert log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertEvent {
    pub id: Option<i64>,
    pub config_id: Option<i64>,
    /// One of `spike | recovery | good_window | stale_data`. Defaults to
    /// `spike` for rows created before Issue #556.
    pub alert_type: String,
    pub severity: String,
    pub peak_fee: i64,
    pub baseline_fee: f64,
    pub spike_ratio: f64,
    pub webhook_url: String,
    pub delivered: bool,
    pub triggered_at: String,
    /// For a `recovery` event, the identity of the spike it resolves
    /// (matches `AlertManager`'s existing spike-identity format).
    /// `None` for spike / good_window / stale_data events.
    pub correlation_id: Option<String>,
}

/// A persisted recommendation row.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub id: Option<i64>,
    pub recommended_fee: i64,
    pub confidence: f64,
    pub target_ledgers: i64,
    pub network_condition: String,
    pub percentile_basis: String,
    pub input_confidence: f64,
    pub input_ledgers: i64,
    pub sample_count: i64,
    pub computed_at: String,
}

/// Static WHERE-clause fragments for the alert-event filters. Each fragment is
/// a fixed string (values are always bound, never interpolated), and the order
/// matches the order callers must bind: severity, delivered, alert_type.
fn alert_event_conditions(
    severity: Option<&str>,
    delivered: Option<bool>,
    alert_type: Option<&str>,
) -> Vec<&'static str> {
    let mut conditions = vec!["1=1"];
    if severity.is_some() {
        conditions.push("severity = ?");
    }
    if delivered.is_some() {
        conditions.push("delivered = ?");
    }
    if alert_type.is_some() {
        conditions.push("alert_type = ?");
    }
    conditions
}

/// Repository for reading and writing fee data to SQLite.
pub struct FeeRepository {
    pool: SqlitePool,
}

impl FeeRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Bulk-insert fee data points in a single transaction.
    /// Timestamps are stored as RFC 3339 strings.
    pub async fn insert_fee_points(&self, points: &[FeeDataPoint]) -> Result<(), sqlx::Error> {
        if points.is_empty() {
            return Ok(());
        }

        let mut tx = self.pool.begin().await?;

        for point in points {
            let timestamp = point.timestamp.to_rfc3339();
            let fee_amount = point.fee_amount as i64;
            let ledger_sequence = point.ledger_sequence as i64;

            sqlx::query(
                "INSERT OR IGNORE INTO fee_data_points
                 (fee_amount, timestamp, transaction_hash, ledger_sequence)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(fee_amount)
            .bind(&timestamp)
            .bind(&point.transaction_hash)
            .bind(ledger_sequence)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Fetch all fee data points with timestamp >= `since`, ordered ascending.
    pub async fn fetch_since(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<FeeDataPoint>, sqlx::Error> {
        let since_str = since.to_rfc3339();

        let rows = sqlx::query(
            "SELECT fee_amount, timestamp, transaction_hash, ledger_sequence
             FROM fee_data_points
             WHERE timestamp >= ?
             ORDER BY timestamp ASC",
        )
        .bind(&since_str)
        .fetch_all(&self.pool)
        .await?;

        let points = rows
            .into_iter()
            .filter_map(|row| {
                use sqlx::Row;
                macro_rules! col {
                    ($col:literal, $T:ty) => {
                        match row.try_get::<$T, _>($col) {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::error!(
                                    "fee_data_points row decode error (column {}): {}",
                                    $col,
                                    e
                                );
                                return None;
                            }
                        }
                    };
                }

                let fee_amount: i64 = col!("fee_amount", i64);
                let timestamp_str: String = col!("timestamp", String);
                let transaction_hash: String = col!("transaction_hash", String);
                let ledger_sequence: i64 = col!("ledger_sequence", i64);

                let timestamp = match DateTime::parse_from_rfc3339(&timestamp_str) {
                    Ok(ts) => ts.with_timezone(&Utc),
                    Err(e) => {
                        tracing::error!(
                            "fee_data_points row: invalid timestamp '{}': {}",
                            timestamp_str,
                            e
                        );
                        return None;
                    }
                };

                Some(FeeDataPoint {
                    fee_amount: fee_amount as u64,
                    timestamp,
                    transaction_hash,
                    ledger_sequence: ledger_sequence as u64,
                })
            })
            .collect();

        Ok(points)
    }

    /// Delete all fee_data_points with timestamp older than `cutoff`.
    /// Returns the number of rows deleted.
    pub async fn prune_older_than(&self, cutoff: DateTime<Utc>) -> Result<u64, sqlx::Error> {
        let cutoff_str = cutoff.to_rfc3339();

        let result = sqlx::query("DELETE FROM fee_data_points WHERE timestamp < ?")
            .bind(&cutoff_str)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// Delete persisted recommendations computed before `cutoff`.
    /// Returns the number of rows removed.
    pub async fn prune_recommendations_older_than(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM recommendations WHERE computed_at < ?")
            .bind(cutoff.to_rfc3339())
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    /// Delete alert events triggered before `cutoff`.
    /// Returns the number of rows removed.
    pub async fn prune_alert_events_older_than(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM alert_events WHERE triggered_at < ?")
            .bind(cutoff.to_rfc3339())
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    // ---- Fee stats snapshots (Issue #550) ----

    /// Idempotently persist a `/fee_stats` snapshot.
    ///
    /// `fee_stats_snapshots.ledger` is the primary key, so re-polling the
    /// same ledger updates the existing row (`ON CONFLICT (ledger) DO
    /// UPDATE`) instead of inserting a duplicate. Returns the number of
    /// rows written (always 1 on success).
    #[allow(dead_code)]
    pub async fn upsert_fee_snapshot(
        &self,
        snapshot: &FeeStatsSnapshot,
    ) -> Result<u64, sqlx::Error> {
        let captured_at = snapshot.timestamp.to_rfc3339();

        let result = sqlx::query(
            "INSERT INTO fee_stats_snapshots (
                ledger, base_fee, min_fee_charged, max_fee_charged, mode_fee_charged,
                mean_fee_charged, median_fee_charged, p10_fee_charged, p95_fee_charged,
                p99_fee_charged, max_fee, ledger_capacity_usage, captured_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (ledger) DO UPDATE SET
                base_fee = excluded.base_fee,
                min_fee_charged = excluded.min_fee_charged,
                max_fee_charged = excluded.max_fee_charged,
                mode_fee_charged = excluded.mode_fee_charged,
                mean_fee_charged = excluded.mean_fee_charged,
                median_fee_charged = excluded.median_fee_charged,
                p10_fee_charged = excluded.p10_fee_charged,
                p95_fee_charged = excluded.p95_fee_charged,
                p99_fee_charged = excluded.p99_fee_charged,
                max_fee = excluded.max_fee,
                ledger_capacity_usage = excluded.ledger_capacity_usage,
                captured_at = excluded.captured_at",
        )
        .bind(snapshot.ledger as i64)
        .bind(snapshot.base_fee as i64)
        .bind(snapshot.min_fee_charged as i64)
        .bind(snapshot.max_fee_charged as i64)
        .bind(snapshot.mode_fee_charged as i64)
        .bind(snapshot.mean_fee_charged)
        .bind(snapshot.median_fee_charged as i64)
        .bind(snapshot.p10_fee_charged as i64)
        .bind(snapshot.p95_fee_charged as i64)
        .bind(snapshot.p99_fee_charged as i64)
        .bind(snapshot.max_fee as i64)
        .bind(snapshot.ledger_capacity_usage)
        .bind(&captured_at)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Fetch all fee stats snapshots with captured_at >= `since`,
    /// ordered by ledger ascending.
    #[allow(dead_code)]
    pub async fn fetch_fee_snapshots_since(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<FeeStatsSnapshot>, sqlx::Error> {
        let since_str = since.to_rfc3339();

        let rows = sqlx::query(
            "SELECT ledger, base_fee, min_fee_charged, max_fee_charged, mode_fee_charged,
                    mean_fee_charged, median_fee_charged, p10_fee_charged, p95_fee_charged,
                    p99_fee_charged, max_fee, ledger_capacity_usage, captured_at
             FROM fee_stats_snapshots
             WHERE captured_at >= ?
             ORDER BY ledger ASC",
        )
        .bind(&since_str)
        .fetch_all(&self.pool)
        .await?;

        let snapshots = rows
            .into_iter()
            .filter_map(|row| {
                use sqlx::Row;
                macro_rules! col {
                    ($col:literal, $T:ty) => {
                        match row.try_get::<$T, _>($col) {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::error!(
                                    "fee_stats_snapshots row decode error (column {}): {}",
                                    $col,
                                    e
                                );
                                return None;
                            }
                        }
                    };
                }

                let ledger: i64 = col!("ledger", i64);
                let base_fee: i64 = col!("base_fee", i64);
                let min_fee_charged: i64 = col!("min_fee_charged", i64);
                let max_fee_charged: i64 = col!("max_fee_charged", i64);
                let mode_fee_charged: i64 = col!("mode_fee_charged", i64);
                let mean_fee_charged: f64 = col!("mean_fee_charged", f64);
                let median_fee_charged: i64 = col!("median_fee_charged", i64);
                let p10_fee_charged: i64 = col!("p10_fee_charged", i64);
                let p95_fee_charged: i64 = col!("p95_fee_charged", i64);
                let p99_fee_charged: i64 = col!("p99_fee_charged", i64);
                let max_fee: i64 = col!("max_fee", i64);
                let ledger_capacity_usage: Option<f64> = col!("ledger_capacity_usage", Option<f64>);
                let captured_at: String = col!("captured_at", String);

                let timestamp = match DateTime::parse_from_rfc3339(&captured_at) {
                    Ok(ts) => ts.with_timezone(&Utc),
                    Err(e) => {
                        tracing::error!(
                            "fee_stats_snapshots row: invalid timestamp '{}': {}",
                            captured_at,
                            e
                        );
                        return None;
                    }
                };

                Some(FeeStatsSnapshot {
                    ledger: ledger as u64,
                    base_fee: base_fee as u64,
                    min_fee_charged: min_fee_charged as u64,
                    max_fee_charged: max_fee_charged as u64,
                    mode_fee_charged: mode_fee_charged as u64,
                    mean_fee_charged,
                    median_fee_charged: median_fee_charged as u64,
                    p10_fee_charged: p10_fee_charged as u64,
                    p95_fee_charged: p95_fee_charged as u64,
                    p99_fee_charged: p99_fee_charged as u64,
                    max_fee: max_fee as u64,
                    ledger_capacity_usage,
                    timestamp,
                })
            })
            .collect();

        Ok(snapshots)
    }

    /// Delete all fee_stats_snapshots captured before `cutoff`.
    /// Returns the number of rows deleted.
    #[allow(dead_code)]
    pub async fn prune_fee_snapshots_older_than(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, sqlx::Error> {
        let cutoff_str = cutoff.to_rfc3339();

        let result = sqlx::query("DELETE FROM fee_stats_snapshots WHERE captured_at < ?")
            .bind(&cutoff_str)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    // ---- Alert config CRUD ----

    /// Insert a new alert webhook config. Returns the new row id.
    #[allow(dead_code)]
    pub async fn insert_alert_config(
        &self,
        webhook_url: &str,
        threshold: &str,
    ) -> Result<i64, sqlx::Error> {
        let result =
            sqlx::query("INSERT INTO alert_configs (webhook_url, threshold) VALUES (?, ?)")
                .bind(webhook_url)
                .bind(threshold)
                .execute(&self.pool)
                .await?;

        Ok(result.last_insert_rowid())
    }

    /// Insert a new alert webhook config with an explicit alert type.
    /// Caller is expected to validate `alert_type` against
    /// `VALID_ALERT_TYPES` first.
    pub async fn insert_alert_config_typed(
        &self,
        webhook_url: &str,
        threshold: &str,
        alert_type: &str,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO alert_configs (webhook_url, threshold, alert_type) VALUES (?, ?, ?)",
        )
        .bind(webhook_url)
        .bind(threshold)
        .bind(alert_type)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// List all alert configs (both enabled and disabled).
    pub async fn list_alert_configs(&self) -> Result<Vec<AlertConfig>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, webhook_url, threshold, alert_type, enabled, created_at FROM alert_configs ORDER BY id ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        let configs = rows
            .into_iter()
            .filter_map(|row| {
                use sqlx::Row;
                macro_rules! col {
                    ($col:literal, $T:ty) => {
                        match row.try_get::<$T, _>($col) {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::error!(
                                    "alert_configs row decode error (column {}): {}",
                                    $col,
                                    e
                                );
                                return None;
                            }
                        }
                    };
                }

                let id: i64 = col!("id", i64);
                let webhook_url: String = col!("webhook_url", String);
                let threshold: String = col!("threshold", String);
                let alert_type: String = col!("alert_type", String);
                let enabled: i64 = col!("enabled", i64);
                let created_at: String = col!("created_at", String);

                Some(AlertConfig {
                    id,
                    webhook_url,
                    threshold,
                    alert_type,
                    enabled: enabled != 0,
                    created_at,
                })
            })
            .collect();

        Ok(configs)
    }

    /// Update threshold and/or enabled state for an alert config.
    /// Returns `true` if a row was updated, `false` if id not found.
    #[allow(dead_code)]
    pub async fn update_alert_config(
        &self,
        id: i64,
        threshold: &str,
        enabled: bool,
    ) -> Result<bool, sqlx::Error> {
        let enabled_int: i64 = if enabled { 1 } else { 0 };

        let result = sqlx::query(
            "UPDATE alert_configs SET threshold = ?, enabled = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(threshold)
        .bind(enabled_int)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Update threshold, enabled state, and alert type for an alert config.
    /// Returns `true` if a row was updated, `false` if id not found.
    pub async fn update_alert_config_full(
        &self,
        id: i64,
        threshold: &str,
        enabled: bool,
        alert_type: &str,
    ) -> Result<bool, sqlx::Error> {
        let enabled_int: i64 = if enabled { 1 } else { 0 };

        let result = sqlx::query(
            "UPDATE alert_configs SET threshold = ?, enabled = ?, alert_type = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(threshold)
        .bind(enabled_int)
        .bind(alert_type)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Soft-delete an alert config by setting enabled = 0.
    /// Returns `true` if a row was found and updated.
    pub async fn delete_alert_config(&self, id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "UPDATE alert_configs SET enabled = 0, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    // ---- Alert event logging ----

    /// Log a fired alert event (success or failure).
    #[allow(dead_code)]
    pub async fn log_alert_event(&self, event: &AlertEvent) -> Result<(), sqlx::Error> {
        let delivered_int: i64 = if event.delivered { 1 } else { 0 };

        sqlx::query(
            "INSERT INTO alert_events
             (config_id, alert_type, severity, peak_fee, baseline_fee, spike_ratio, webhook_url, delivered, triggered_at, correlation_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(event.config_id)
        .bind(&event.alert_type)
        .bind(&event.severity)
        .bind(event.peak_fee)
        .bind(event.baseline_fee)
        .bind(event.spike_ratio)
        .bind(&event.webhook_url)
        .bind(delivered_int)
        .bind(&event.triggered_at)
        .bind(&event.correlation_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Query alert history, newest first. Delegates to
    /// [`Self::query_alert_history_by_type`] with no alert-type filter.
    #[allow(dead_code)]
    pub async fn query_alert_history(
        &self,
        limit: i64,
        severity_filter: Option<&str>,
        delivered_filter: Option<bool>,
    ) -> Result<Vec<AlertEvent>, sqlx::Error> {
        self.query_alert_history_by_type(limit, severity_filter, delivered_filter, None)
            .await
    }

    /// Count alert events matching optional filters (for pagination totals).
    /// Delegates to [`Self::count_alert_events_by_type`].
    #[allow(dead_code)]
    pub async fn count_alert_events(
        &self,
        severity_filter: Option<&str>,
        delivered_filter: Option<bool>,
    ) -> Result<i64, sqlx::Error> {
        self.count_alert_events_by_type(severity_filter, delivered_filter, None)
            .await
    }

    /// Query alert history filtered additionally by alert_type. Same
    /// pagination/filter semantics as `query_alert_history`.
    pub async fn query_alert_history_by_type(
        &self,
        limit: i64,
        severity_filter: Option<&str>,
        delivered_filter: Option<bool>,
        alert_type_filter: Option<&str>,
    ) -> Result<Vec<AlertEvent>, sqlx::Error> {
        let limit = limit.clamp(1, 100);

        let conditions =
            alert_event_conditions(severity_filter, delivered_filter, alert_type_filter);

        let sql = format!(
            "SELECT id, config_id, alert_type, severity, peak_fee, baseline_fee, spike_ratio, webhook_url, delivered, triggered_at, correlation_id
             FROM alert_events
             WHERE {}
             ORDER BY triggered_at DESC
             LIMIT ?",
            conditions.join(" AND ")
        );

        let rows = {
            let mut q = sqlx::query(&sql);
            if let Some(sev) = severity_filter {
                q = q.bind(sev);
            }
            if let Some(del) = delivered_filter {
                q = q.bind(if del { 1i64 } else { 0i64 });
            }
            if let Some(at) = alert_type_filter {
                q = q.bind(at);
            }
            q.bind(limit).fetch_all(&self.pool).await?
        };

        let events = rows
            .into_iter()
            .filter_map(|row| {
                use sqlx::Row;
                macro_rules! col {
                    ($col:literal, $T:ty) => {
                        match row.try_get::<$T, _>($col) {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::error!(
                                    "alert_events row decode error (column {}): {}",
                                    $col,
                                    e
                                );
                                return None;
                            }
                        }
                    };
                }

                let id: i64 = col!("id", i64);
                let config_id: Option<i64> = col!("config_id", Option<i64>);
                let alert_type: String = col!("alert_type", String);
                let severity: String = col!("severity", String);
                let peak_fee: i64 = col!("peak_fee", i64);
                let baseline_fee: f64 = col!("baseline_fee", f64);
                let spike_ratio: f64 = col!("spike_ratio", f64);
                let webhook_url: String = col!("webhook_url", String);
                let delivered: i64 = col!("delivered", i64);
                let triggered_at: String = col!("triggered_at", String);
                let correlation_id: Option<String> = col!("correlation_id", Option<String>);

                Some(AlertEvent {
                    id: Some(id),
                    config_id,
                    alert_type,
                    severity,
                    peak_fee,
                    baseline_fee,
                    spike_ratio,
                    webhook_url,
                    delivered: delivered != 0,
                    triggered_at,
                    correlation_id,
                })
            })
            .collect();

        Ok(events)
    }

    /// Count alert events matching optional filters including alert_type.
    pub async fn count_alert_events_by_type(
        &self,
        severity_filter: Option<&str>,
        delivered_filter: Option<bool>,
        alert_type_filter: Option<&str>,
    ) -> Result<i64, sqlx::Error> {
        let conditions =
            alert_event_conditions(severity_filter, delivered_filter, alert_type_filter);

        let sql = format!(
            "SELECT COUNT(*) as cnt FROM alert_events WHERE {}",
            conditions.join(" AND ")
        );

        let row = {
            let mut q = sqlx::query(&sql);
            if let Some(sev) = severity_filter {
                q = q.bind(sev);
            }
            if let Some(del) = delivered_filter {
                q = q.bind(if del { 1i64 } else { 0i64 });
            }
            if let Some(at) = alert_type_filter {
                q = q.bind(at);
            }
            q.fetch_one(&self.pool).await?
        };

        use sqlx::Row;
        let count: i64 = row.try_get("cnt").map_err(|e| {
            tracing::error!("Failed to decode COUNT(*) result from alert_events: {}", e);
            e
        })?;
        Ok(count)
    }

    // ---- Recommendations ----

    /// Persist a recommendation row. Returns the new row id.
    #[allow(dead_code)]
    pub async fn insert_recommendation(&self, rec: &Recommendation) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO recommendations
             (recommended_fee, confidence, target_ledgers, network_condition,
              percentile_basis, input_confidence, input_ledgers, sample_count, computed_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(rec.recommended_fee)
        .bind(rec.confidence)
        .bind(rec.target_ledgers)
        .bind(&rec.network_condition)
        .bind(&rec.percentile_basis)
        .bind(rec.input_confidence)
        .bind(rec.input_ledgers)
        .bind(rec.sample_count)
        .bind(&rec.computed_at)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Return fee amounts from `fee_data_points` with timestamp >= `since`, sorted ascending.
    #[allow(dead_code)]
    pub async fn get_fees_since(&self, since: DateTime<Utc>) -> Result<Vec<u64>, sqlx::Error> {
        let since_str = since.to_rfc3339();

        let rows = sqlx::query(
            "SELECT fee_amount FROM fee_data_points WHERE timestamp >= ? ORDER BY fee_amount ASC",
        )
        .bind(&since_str)
        .fetch_all(&self.pool)
        .await?;

        let fees = rows
            .into_iter()
            .filter_map(|row| {
                use sqlx::Row;
                row.try_get::<i64, _>("fee_amount").map(|v| v as u64).ok()
            })
            .collect();

        Ok(fees)
    }

    /// Query the most recent `limit` recommendation rows, newest first.
    #[allow(dead_code)]
    pub async fn query_recent_recommendations(
        &self,
        limit: i64,
    ) -> Result<Vec<Recommendation>, sqlx::Error> {
        let limit = limit.clamp(1, 100);

        let rows = sqlx::query(
            "SELECT id, recommended_fee, confidence, target_ledgers, network_condition,
                    percentile_basis, input_confidence, input_ledgers, sample_count, computed_at
             FROM recommendations
             ORDER BY computed_at DESC
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let recs = rows
            .into_iter()
            .filter_map(|row| {
                use sqlx::Row;
                macro_rules! col {
                    ($col:literal, $T:ty) => {
                        match row.try_get::<$T, _>($col) {
                            Ok(v) => v,
                            Err(e) => {
                                tracing::error!(
                                    "recommendations row decode error (column {}): {}",
                                    $col,
                                    e
                                );
                                return None;
                            }
                        }
                    };
                }

                Some(Recommendation {
                    id: Some(col!("id", i64)),
                    recommended_fee: col!("recommended_fee", i64),
                    confidence: col!("confidence", f64),
                    target_ledgers: col!("target_ledgers", i64),
                    network_condition: col!("network_condition", String),
                    percentile_basis: col!("percentile_basis", String),
                    input_confidence: col!("input_confidence", f64),
                    input_ledgers: col!("input_ledgers", i64),
                    sample_count: col!("sample_count", i64),
                    computed_at: col!("computed_at", String),
                })
            })
            .collect();

        Ok(recs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    use crate::db::create_pool;

    async fn make_repo() -> FeeRepository {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        FeeRepository::new(pool)
    }

    fn make_point(fee_amount: u64, seconds_ago: i64) -> FeeDataPoint {
        FeeDataPoint {
            fee_amount,
            timestamp: Utc::now() - Duration::seconds(seconds_ago),
            transaction_hash: format!("hash_{}", fee_amount),
            ledger_sequence: 1,
        }
    }

    #[tokio::test]
    async fn insert_and_fetch_roundtrip() {
        let repo = make_repo().await;
        let points = vec![
            make_point(100, 300),
            make_point(200, 200),
            make_point(300, 100),
        ];

        repo.insert_fee_points(&points).await.unwrap();

        let since = Utc::now() - Duration::seconds(400);
        let fetched = repo.fetch_since(since).await.unwrap();

        assert_eq!(fetched.len(), 3);
        assert_eq!(fetched[0].fee_amount, 100);
        assert_eq!(fetched[1].fee_amount, 200);
        assert_eq!(fetched[2].fee_amount, 300);
    }

    #[tokio::test]
    async fn fetch_since_filters_old_points() {
        let repo = make_repo().await;
        let points = vec![
            make_point(100, 7200), // 2 hours ago — outside window
            make_point(200, 1800), // 30 min ago — inside window
            make_point(300, 600),  // 10 min ago — inside window
        ];

        repo.insert_fee_points(&points).await.unwrap();

        let since = Utc::now() - Duration::hours(1);
        let fetched = repo.fetch_since(since).await.unwrap();

        assert_eq!(fetched.len(), 2);
        assert_eq!(fetched[0].fee_amount, 200);
        assert_eq!(fetched[1].fee_amount, 300);
    }

    #[tokio::test]
    async fn insert_empty_slice_is_ok() {
        let repo = make_repo().await;
        let result = repo.insert_fee_points(&[]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn prune_older_than_removes_old_rows() {
        let repo = make_repo().await;
        let points = vec![
            make_point(100, 7200), // 2 hours ago — outside window, should be pruned
            make_point(200, 1800), // 30 min ago — clearly inside window, kept
            make_point(300, 600),  // 10 min ago — inside window, kept
        ];

        repo.insert_fee_points(&points).await.unwrap();

        let cutoff = Utc::now() - Duration::hours(1);
        let deleted = repo.prune_older_than(cutoff).await.unwrap();

        assert_eq!(deleted, 1);

        let remaining = repo
            .fetch_since(Utc::now() - Duration::days(1))
            .await
            .unwrap();
        assert_eq!(remaining.len(), 2);
    }

    #[tokio::test]
    async fn prune_older_than_returns_zero_when_nothing_to_prune() {
        let repo = make_repo().await;
        let points = vec![make_point(100, 60)]; // 1 min ago

        repo.insert_fee_points(&points).await.unwrap();

        let cutoff = Utc::now() - Duration::hours(1);
        let deleted = repo.prune_older_than(cutoff).await.unwrap();

        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn fetch_since_returns_empty_when_no_data() {
        let repo = make_repo().await;
        let fetched = repo
            .fetch_since(Utc::now() - Duration::hours(24))
            .await
            .unwrap();
        assert!(fetched.is_empty());
    }
}
#[cfg(test)]
mod alert_tests {
    use super::*;
    use crate::db::create_pool;

    async fn make_repo() -> FeeRepository {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        FeeRepository::new(pool)
    }

    #[tokio::test]
    async fn insert_and_list_alert_config() {
        let repo = make_repo().await;
        let id = repo
            .insert_alert_config("https://hooks.example.com/webhook", "Major")
            .await
            .unwrap();
        assert!(id > 0);
        let configs = repo.list_alert_configs().await.unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].webhook_url, "https://hooks.example.com/webhook");
        assert_eq!(configs[0].threshold, "Major");
        // Backward compatibility: rows inserted via the original 2-arg
        // constructor default to alert_type = 'spike' (Issue #556).
        assert_eq!(configs[0].alert_type, "spike");
        assert!(configs[0].enabled);
    }

    #[tokio::test]
    async fn insert_alert_config_typed_sets_alert_type() {
        let repo = make_repo().await;
        let id = repo
            .insert_alert_config_typed("https://hooks.example.com/typed", "Major", "stale_data")
            .await
            .unwrap();
        assert!(id > 0);
        let configs = repo.list_alert_configs().await.unwrap();
        assert_eq!(configs[0].alert_type, "stale_data");
    }

    #[tokio::test]
    async fn update_alert_config_full_changes_alert_type() {
        let repo = make_repo().await;
        let id = repo
            .insert_alert_config("https://hooks.example.com/c", "Minor")
            .await
            .unwrap();
        let updated = repo
            .update_alert_config_full(id, "Major", true, "good_window")
            .await
            .unwrap();
        assert!(updated);
        let configs = repo.list_alert_configs().await.unwrap();
        assert_eq!(configs[0].alert_type, "good_window");
    }

    #[tokio::test]
    async fn update_alert_config_changes_threshold_and_enabled() {
        let repo = make_repo().await;
        let id = repo
            .insert_alert_config("https://hooks.example.com/a", "Minor")
            .await
            .unwrap();
        let updated = repo
            .update_alert_config(id, "Critical", false)
            .await
            .unwrap();
        assert!(updated);
        let configs = repo.list_alert_configs().await.unwrap();
        assert_eq!(configs[0].threshold, "Critical");
        assert!(!configs[0].enabled);
    }

    #[tokio::test]
    async fn update_alert_config_returns_false_for_missing_id() {
        let repo = make_repo().await;
        let updated = repo.update_alert_config(9999, "Major", true).await.unwrap();
        assert!(!updated);
    }

    #[tokio::test]
    async fn delete_alert_config_soft_deletes() {
        let repo = make_repo().await;
        let id = repo
            .insert_alert_config("https://hooks.example.com/b", "Major")
            .await
            .unwrap();
        let deleted = repo.delete_alert_config(id).await.unwrap();
        assert!(deleted);
        let configs = repo.list_alert_configs().await.unwrap();
        assert_eq!(configs.len(), 1);
        assert!(!configs[0].enabled);
    }

    #[tokio::test]
    async fn delete_alert_config_returns_false_for_missing_id() {
        let repo = make_repo().await;
        let deleted = repo.delete_alert_config(9999).await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn full_crud_cycle() {
        let repo = make_repo().await;
        let id = repo
            .insert_alert_config("https://hooks.example.com/cycle", "Minor")
            .await
            .unwrap();
        let configs = repo.list_alert_configs().await.unwrap();
        assert_eq!(configs.len(), 1);
        repo.update_alert_config(id, "Major", true).await.unwrap();
        let configs = repo.list_alert_configs().await.unwrap();
        assert_eq!(configs[0].threshold, "Major");
        repo.delete_alert_config(id).await.unwrap();
        let configs = repo.list_alert_configs().await.unwrap();
        assert!(!configs[0].enabled);
    }
}

#[cfg(test)]
mod alert_event_tests {
    use super::*;
    use crate::db::create_pool;

    async fn make_repo() -> FeeRepository {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        FeeRepository::new(pool)
    }

    fn make_event(severity: &str, delivered: bool) -> AlertEvent {
        AlertEvent {
            id: None,
            config_id: None,
            alert_type: "spike".to_string(),
            severity: severity.to_string(),
            peak_fee: 8000,
            baseline_fee: 130.5,
            spike_ratio: 61.3,
            webhook_url: "https://hooks.example.com/test".to_string(),
            delivered,
            triggered_at: chrono::Utc::now().to_rfc3339(),
            correlation_id: None,
        }
    }

    #[tokio::test]
    async fn log_and_query_five_events() {
        let repo = make_repo().await;
        for _ in 0..5 {
            repo.log_alert_event(&make_event("Major", true))
                .await
                .unwrap();
        }
        let events = repo.query_alert_history(20, None, None).await.unwrap();
        assert_eq!(events.len(), 5);
    }

    #[tokio::test]
    async fn filter_by_severity() {
        let repo = make_repo().await;
        repo.log_alert_event(&make_event("Minor", true))
            .await
            .unwrap();
        repo.log_alert_event(&make_event("Major", true))
            .await
            .unwrap();
        repo.log_alert_event(&make_event("Critical", false))
            .await
            .unwrap();

        let major = repo
            .query_alert_history(20, Some("Major"), None)
            .await
            .unwrap();
        assert_eq!(major.len(), 1);
        assert_eq!(major[0].severity, "Major");

        let critical = repo
            .query_alert_history(20, Some("Critical"), None)
            .await
            .unwrap();
        assert_eq!(critical.len(), 1);
        assert_eq!(critical[0].severity, "Critical");
    }

    #[tokio::test]
    async fn filter_by_delivered() {
        let repo = make_repo().await;
        repo.log_alert_event(&make_event("Major", true))
            .await
            .unwrap();
        repo.log_alert_event(&make_event("Major", false))
            .await
            .unwrap();
        repo.log_alert_event(&make_event("Major", true))
            .await
            .unwrap();

        let delivered = repo
            .query_alert_history(20, None, Some(true))
            .await
            .unwrap();
        assert_eq!(delivered.len(), 2);

        let failed = repo
            .query_alert_history(20, None, Some(false))
            .await
            .unwrap();
        assert_eq!(failed.len(), 1);
    }

    #[tokio::test]
    async fn limit_clamped_to_100() {
        let repo = make_repo().await;
        for _ in 0..5 {
            repo.log_alert_event(&make_event("Major", true))
                .await
                .unwrap();
        }
        // Requesting 999 should be clamped to 100; still only 5 rows in DB
        let events = repo.query_alert_history(999, None, None).await.unwrap();
        assert_eq!(events.len(), 5);
    }

    #[tokio::test]
    async fn count_alert_events_total() {
        let repo = make_repo().await;
        for _ in 0..5 {
            repo.log_alert_event(&make_event("Major", true))
                .await
                .unwrap();
        }
        let total = repo.count_alert_events(None, None).await.unwrap();
        assert_eq!(total, 5);
    }

    #[tokio::test]
    async fn count_alert_events_filtered() {
        let repo = make_repo().await;
        repo.log_alert_event(&make_event("Minor", true))
            .await
            .unwrap();
        repo.log_alert_event(&make_event("Major", true))
            .await
            .unwrap();
        repo.log_alert_event(&make_event("Critical", false))
            .await
            .unwrap();

        let major_count = repo.count_alert_events(Some("Major"), None).await.unwrap();
        assert_eq!(major_count, 1);

        let delivered_count = repo.count_alert_events(None, Some(true)).await.unwrap();
        assert_eq!(delivered_count, 2);

        let critical_failed = repo
            .count_alert_events(Some("Critical"), Some(false))
            .await
            .unwrap();
        assert_eq!(critical_failed, 1);
    }

    #[tokio::test]
    async fn logged_event_has_assigned_id() {
        let repo = make_repo().await;
        repo.log_alert_event(&make_event("Major", true))
            .await
            .unwrap();
        let events = repo.query_alert_history(1, None, None).await.unwrap();
        assert!(events[0].id.is_some());
        assert!(events[0].id.unwrap() > 0);
    }
}

#[cfg(test)]
mod fee_stats_snapshot_tests {
    use super::*;
    use chrono::Duration;

    use crate::db::create_pool;

    async fn make_repo() -> FeeRepository {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        FeeRepository::new(pool)
    }

    fn make_snapshot(ledger: u64, base_fee: u64, mode_fee: u64) -> FeeStatsSnapshot {
        FeeStatsSnapshot {
            ledger,
            base_fee,
            min_fee_charged: 100,
            max_fee_charged: 5000,
            mode_fee_charged: mode_fee,
            mean_fee_charged: 250.75,
            median_fee_charged: 200,
            p10_fee_charged: 100,
            p95_fee_charged: 800,
            p99_fee_charged: 1200,
            max_fee: 10_000,
            ledger_capacity_usage: Some(0.97),
            timestamp: Utc::now(),
        }
    }

    #[tokio::test]
    async fn upsert_fee_snapshot_inserts_then_updates_same_ledger() {
        let repo = make_repo().await;

        repo.upsert_fee_snapshot(&make_snapshot(50_000_001, 100, 213))
            .await
            .unwrap();
        // Same ledger again — must UPDATE, not duplicate.
        repo.upsert_fee_snapshot(&make_snapshot(50_000_001, 200, 300))
            .await
            .unwrap();

        use sqlx::Row;
        let row = sqlx::query(
            "SELECT COUNT(*) AS cnt, MAX(base_fee) AS base_fee, MAX(mode_fee_charged) AS mode_fee
             FROM fee_stats_snapshots WHERE ledger = ?",
        )
        .bind(50_000_001i64)
        .fetch_one(&repo.pool)
        .await
        .unwrap();

        assert_eq!(row.try_get::<i64, _>("cnt").unwrap(), 1);
        assert_eq!(row.try_get::<i64, _>("base_fee").unwrap(), 200);
        assert_eq!(row.try_get::<i64, _>("mode_fee").unwrap(), 300);
    }

    #[tokio::test]
    async fn upsert_fee_snapshot_keeps_distinct_ledgers_separate() {
        let repo = make_repo().await;

        repo.upsert_fee_snapshot(&make_snapshot(1, 100, 213))
            .await
            .unwrap();
        repo.upsert_fee_snapshot(&make_snapshot(2, 100, 256))
            .await
            .unwrap();

        let snapshots = repo
            .fetch_fee_snapshots_since(Utc::now() - Duration::hours(1))
            .await
            .unwrap();
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].ledger, 1);
        assert_eq!(snapshots[0].mode_fee_charged, 213);
        assert_eq!(snapshots[1].ledger, 2);
        assert_eq!(snapshots[1].mode_fee_charged, 256);
        assert!((snapshots[0].mean_fee_charged - 250.75).abs() < f64::EPSILON);
        assert!((snapshots[0].ledger_capacity_usage.unwrap() - 0.97).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn prune_fee_snapshots_older_than_removes_only_old_rows() {
        let repo = make_repo().await;

        let mut old = make_snapshot(1, 100, 213);
        old.timestamp = Utc::now() - Duration::hours(2);
        let fresh = make_snapshot(2, 100, 256);

        repo.upsert_fee_snapshot(&old).await.unwrap();
        repo.upsert_fee_snapshot(&fresh).await.unwrap();

        let deleted = repo
            .prune_fee_snapshots_older_than(Utc::now() - Duration::hours(1))
            .await
            .unwrap();
        assert_eq!(deleted, 1);

        let remaining = repo
            .fetch_fee_snapshots_since(Utc::now() - Duration::days(1))
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].ledger, 2);
    }
}
