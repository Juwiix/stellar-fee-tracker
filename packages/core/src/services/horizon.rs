use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Clone)]
pub struct HorizonClient {
    base_url: String,
    http: Client,
}

impl HorizonClient {
    pub fn new(base_url: String) -> Self {
        let http = Client::builder()
            .no_proxy()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { base_url, http }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Expose the shared HTTP client for adapters that need to make
    /// additional requests (e.g. `HorizonFeeDataProvider`).
    pub(crate) fn http_client(&self) -> &Client {
        &self.http
    }

    pub async fn fetch_account_transactions(
        &self,
        account_id: &str,
        limit: u32,
    ) -> Result<Vec<crate::insights::types::FeeDataPoint>, AppError> {
        Ok(self
            .fetch_account_transaction_records(account_id, limit)
            .await?
            .into_iter()
            .map(|(point, _successful)| point)
            .collect())
    }

    /// Like [`Self::fetch_account_transactions`] but also returns Horizon's
    /// `successful` flag for each transaction.
    pub async fn fetch_account_transaction_records(
        &self,
        account_id: &str,
        limit: u32,
    ) -> Result<Vec<(crate::insights::types::FeeDataPoint, bool)>, AppError> {
        let url = format!(
            "{}/accounts/{}/transactions?limit={}&order=desc",
            self.base_url, account_id, limit
        );
        let response = self
            .http
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| {
                AppError::Network(format!("Failed to fetch account transactions: {}", e))
            })?;

        if !response.status().is_success() {
            return Err(AppError::Network(format!(
                "Horizon returned HTTP {}",
                response.status()
            )));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| AppError::Parse(format!("Failed to parse account transactions: {}", e)))?;

        let records = body["_embedded"]["records"]
            .as_array()
            .ok_or_else(|| AppError::Parse("Missing _embedded.records in response".to_string()))?;

        let points = records
            .iter()
            .filter_map(|r| {
                let hash = r["hash"].as_str()?.to_string();
                let fee_charged = r["fee_charged"].as_str()?.parse::<u64>().ok()?;
                let ledger = r["ledger"].as_str()?.parse::<u64>().ok()?;
                let created_at = r["created_at"].as_str()?.to_string();
                let successful = r["successful"].as_bool()?;
                Some((
                    crate::insights::types::FeeDataPoint {
                        fee_amount: fee_charged,
                        timestamp: chrono::DateTime::parse_from_rfc3339(&created_at)
                            .ok()?
                            .with_timezone(&chrono::Utc),
                        transaction_hash: hash,
                        ledger_sequence: ledger,
                    },
                    successful,
                ))
            })
            .collect();

        Ok(points)
    }
}

#[derive(Debug, Deserialize)]
pub struct HorizonFeeStats {
    pub last_ledger_base_fee: String,
    pub fee_charged: FeeCharged,
}

#[derive(Debug, Deserialize)]
pub struct FeeCharged {
    pub min: String,
    pub max: String,
    /// Horizon exposes the most-common fee as `"mode"` in fee_stats.
    /// We surface it as `avg_fee` in the public API (typical fee charged).
    #[serde(rename = "mode")]
    pub avg: String,
    pub p10: String,
    pub p20: String,
    pub p30: String,
    pub p40: String,
    pub p50: String,
    pub p60: String,
    pub p70: String,
    pub p80: String,
    pub p90: String,
    pub p95: String,
    pub p99: String,
}

/// Complete `/fee_stats` payload as exposed by Horizon (Issue #550).
///
/// Unlike [`HorizonFeeStats`] — which only carries the subset surfaced by
/// the public API — this captures every field Horizon returns, including
/// the ledger number, capacity usage, mean/median fee charged and the
/// `max_fee` distribution.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeStatsResponse {
    pub last_ledger: String,
    pub last_ledger_base_fee: String,
    pub ledger_capacity_usage: Option<String>,
    pub fee_charged: FeeChargedStats,
    pub max_fee: MaxFeeStats,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeChargedStats {
    pub min: String,
    pub max: String,
    pub mode: String,
    pub mean: String,
    pub median: String,
    pub p10: String,
    pub p20: String,
    pub p30: String,
    pub p40: String,
    pub p50: String,
    pub p60: String,
    pub p70: String,
    pub p80: String,
    pub p90: String,
    pub p95: String,
    pub p99: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaxFeeStats {
    pub min: String,
    pub max: String,
    pub mode: String,
    pub mean: String,
    pub median: String,
}

impl HorizonClient {
    pub async fn fetch_fee_stats(&self) -> Result<HorizonFeeStats, AppError> {
        let url = format!("{}/fee_stats", self.base_url);

        let response = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|err| AppError::Network(err.to_string()))?;

        if !response.status().is_success() {
            return Err(AppError::Network(format!(
                "Horizon returned HTTP {}",
                response.status()
            )));
        }

        let stats = response
            .json::<HorizonFeeStats>()
            .await
            .map_err(|err| AppError::Parse(err.to_string()))?;

        Ok(stats)
    }

    /// Fetch the complete `/fee_stats` payload from Horizon (Issue #550).
    ///
    /// [`HorizonClient::fetch_fee_stats`] returns only the subset needed by
    /// the public `/fees/current` API; this method surfaces every field —
    /// ledger number, capacity usage, mean/median fee charged and the
    /// `max_fee` distribution — required for accurate fee modelling.
    #[allow(dead_code)]
    pub async fn fetch_fee_stats_full(&self) -> Result<FeeStatsResponse, AppError> {
        let url = format!("{}/fee_stats", self.base_url);

        let response = self
            .http
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| AppError::Network(format!("Failed to fetch fee_stats: {}", e)))?;

        if !response.status().is_success() {
            return Err(AppError::Network(format!(
                "Horizon returned HTTP {}",
                response.status()
            )));
        }

        let stats = response
            .json::<FeeStatsResponse>()
            .await
            .map_err(|e| AppError::Parse(format!("Failed to parse fee_stats: {}", e)))?;

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizon_client_base_url_is_stored() {
        let client = HorizonClient::new("https://horizon-testnet.stellar.org".into());
        assert_eq!(client.base_url(), "https://horizon-testnet.stellar.org");
    }

    #[test]
    fn fee_charged_deserialises_all_percentile_fields() {
        let json = r#"{
            "min": "100",
            "max": "5000",
            "mode": "213",
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
        }"#;
        let fc: FeeCharged = serde_json::from_str(json).unwrap();
        assert_eq!(fc.min, "100");
        assert_eq!(fc.max, "5000");
        assert_eq!(fc.avg, "213");
        assert_eq!(fc.p10, "100");
        assert_eq!(fc.p20, "100");
        assert_eq!(fc.p30, "120");
        assert_eq!(fc.p40, "140");
        assert_eq!(fc.p50, "150");
        assert_eq!(fc.p60, "200");
        assert_eq!(fc.p70, "300");
        assert_eq!(fc.p80, "400");
        assert_eq!(fc.p90, "500");
        assert_eq!(fc.p95, "800");
        assert_eq!(fc.p99, "1200");
    }

    #[test]
    fn horizon_fee_stats_deserialises_with_percentiles() {
        let json = r#"{
            "last_ledger_base_fee": "100",
            "fee_charged": {
                "min": "100",
                "max": "5000",
                "mode": "213",
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
            }
        }"#;
        let stats: HorizonFeeStats = serde_json::from_str(json).unwrap();
        assert_eq!(stats.last_ledger_base_fee, "100");
        assert_eq!(stats.fee_charged.avg, "213");
        assert_eq!(stats.fee_charged.p50, "150");
        assert_eq!(stats.fee_charged.p95, "800");
        assert_eq!(stats.fee_charged.p99, "1200");
    }

    fn full_fee_stats_json() -> String {
        r#"{
            "last_ledger": "50000001",
            "last_ledger_base_fee": "100",
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
        }"#
        .to_string()
    }

    #[test]
    fn fee_stats_response_deserialises_full_payload() {
        let stats: FeeStatsResponse = serde_json::from_str(&full_fee_stats_json()).unwrap();
        assert_eq!(stats.last_ledger, "50000001");
        assert_eq!(stats.last_ledger_base_fee, "100");
        assert_eq!(
            stats.ledger_capacity_usage.as_deref(),
            Some("0.97"),
            "capacity usage should be captured"
        );
        assert_eq!(stats.fee_charged.mode, "213");
        assert_eq!(stats.fee_charged.mean, "250.75");
        assert_eq!(stats.fee_charged.median, "200");
        assert_eq!(stats.fee_charged.p95, "800");
        assert_eq!(stats.max_fee.max, "10000");
        assert_eq!(stats.max_fee.median, "10000");
    }

    #[test]
    fn fee_stats_response_allows_missing_capacity_usage() {
        let json = full_fee_stats_json().replace("\"0.97\"", "null");
        let stats: FeeStatsResponse = serde_json::from_str(&json).unwrap();
        assert!(stats.ledger_capacity_usage.is_none());
    }
}
