use std::sync::Arc;

use axum::{
    extract::State,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;

use crate::error::AppError;
use crate::metrics::AppMetrics;
use crate::middleware::validation::validate_recommend_request;
use crate::recommendation::engine::FeeRecommendationEngine;
use crate::recommendation::types::{
    RecommendHistoryEntry, RecommendHistoryResponse, RecommendRequest, RecommendResponse, Urgency,
};
use crate::repository::FeeRepository;

pub type RecommendationState = Arc<RecommendationApiState>;

pub struct RecommendationApiState {
    pub engine: FeeRecommendationEngine,
    pub metrics: Option<Arc<AppMetrics>>,
    pub repository: Option<Arc<FeeRepository>>,
}

#[derive(serde::Serialize)]
pub struct FeeRecommendResponse {
    pub recommended_fee: u64,
    pub congestion: String,
    pub basis: String,
    /// Lowest fee among the most recent 100 observed transactions (not the
    /// network base fee).
    pub min_fee: u64,
    pub avg_fee: u64,
    pub timestamp: String,
}

pub async fn recommend(
    State(state): State<RecommendationState>,
    Json(body): Json<RecommendRequest>,
) -> Result<Json<RecommendResponse>, Response> {
    validate_recommend_request(&body)
        .map_err(|(status, err_json)| (status, Json(err_json)).into_response())?;

    let result = match state.engine.recommend(&body).await {
        Ok(result) => result,
        Err(err) => {
            if let Some(metrics) = &state.metrics {
                metrics.recommendation_errors_total.inc();
            }
            return Err(err.into_response());
        }
    };

    if let Some(metrics) = &state.metrics {
        metrics.recommendations_total.inc();
    }

    Ok(Json(result))
}

pub async fn get_recommend(
    State(state): State<RecommendationState>,
) -> Result<Json<FeeRecommendResponse>, AppError> {
    let request = RecommendRequest {
        target_ledgers: Some(2),
        urgency: Some(Urgency::Medium),
        max_fee: None,
    };

    if let Err(err) = state.engine.recommend(&request).await {
        if let Some(metrics) = &state.metrics {
            metrics.recommendation_errors_total.inc();
        }
        return Err(err);
    }
    let recent = state.engine.get_last_n_fees(100).await;

    if recent.is_empty() {
        return Err(AppError::Network(
            "No fee data available yet. Wait for the first poll cycle.".to_string(),
        ));
    }

    let mut sorted: Vec<u64> = recent.iter().map(|p| p.fee_amount).collect();
    sorted.sort_unstable();

    let min_fee = sorted[0];
    let sum: u64 = sorted.iter().sum();
    let avg_fee = sum / sorted.len() as u64;

    // Guard against a zero minimum so the ratio never divides by zero.
    let ratio = avg_fee as f64 / min_fee.max(1) as f64;

    let percentile_value = |sorted: &[u64], pct: usize| -> u64 {
        let idx = (pct * sorted.len()).div_ceil(100).min(sorted.len() - 1);
        sorted[idx]
    };

    let (congestion, basis, recommended_fee) = if ratio < 1.5 {
        (
            "LOW".to_string(),
            "p20".to_string(),
            percentile_value(&sorted, 20),
        )
    } else if ratio <= 3.0 {
        (
            "MEDIUM".to_string(),
            "p50".to_string(),
            percentile_value(&sorted, 50),
        )
    } else {
        (
            "HIGH".to_string(),
            "p70".to_string(),
            percentile_value(&sorted, 70),
        )
    };

    Ok(Json(FeeRecommendResponse {
        recommended_fee,
        congestion,
        basis,
        min_fee,
        avg_fee,
        timestamp: Utc::now().to_rfc3339(),
    }))
}

pub async fn recommend_history(
    State(state): State<RecommendationState>,
) -> Result<Json<RecommendHistoryResponse>, AppError> {
    let entries = match &state.repository {
        Some(repository) => repository
            .query_recent_recommendations(50)
            .await
            .map_err(|err| AppError::Unknown(format!("Failed to query recommendations: {}", err)))?
            .into_iter()
            .filter_map(|rec| {
                // Skip rows whose timestamp cannot be parsed rather than
                // reporting them as if they were requested "now".
                let requested_at = match chrono::DateTime::parse_from_rfc3339(&rec.computed_at) {
                    Ok(ts) => ts.with_timezone(&chrono::Utc),
                    Err(err) => {
                        tracing::warn!(
                            "Skipping recommendation {:?} with invalid computed_at '{}': {}",
                            rec.id,
                            rec.computed_at,
                            err
                        );
                        return None;
                    }
                };
                Some(RecommendHistoryEntry {
                    id: rec.id.unwrap_or_default(),
                    requested_at,
                    target_ledgers: rec.target_ledgers as u32,
                    percentile_basis: rec.percentile_basis,
                    recommended_fee: rec.recommended_fee as u64,
                })
            })
            .collect(),
        None => vec![],
    };

    Ok(Json(RecommendHistoryResponse { entries }))
}
