use std::env;

use crate::cli::Cli;
use crate::insights::SpikeSeverity;
use crate::recommendation::types::RecommendationConfig;
use crate::repository::VALID_ALERT_TYPES;

#[derive(Debug, Clone)]
pub struct Config {
    pub stellar_network: StellarNetwork,
    pub horizon_url: String,
    pub poll_interval_seconds: u64,
    pub cache_ttl_seconds: u64,
    pub api_key: Option<String>,
    pub rate_limit_per_minute: u32,
    pub webhook_url: Option<String>,
    pub alert_threshold: SpikeSeverity,
    pub api_port: u16,
    pub allowed_origins: Vec<String>,
    pub retry_attempts: u32,
    pub base_retry_delay_ms: u64,
    pub database_url: String,
    pub storage_retention_days: u64,
    pub recommendation: RecommendationConfig,
    /// Which alert types (from `VALID_ALERT_TYPES`) `AlertManager` will
    /// evaluate each poll tick. Defaults to all four — spike alerting
    /// keeps working exactly as before, and the new Advisor alert types
    /// (Issue #556) are opt-out rather than opt-in.
    pub enabled_alert_types: Vec<String>,
    /// How stale `DataQuality.freshness` must get (in seconds) before a
    /// `stale_data` alert fires. Only consulted when `stale_data` is in
    /// `enabled_alert_types`.
    pub stale_data_threshold_seconds: i64,
}

#[derive(Debug, Clone)]
pub enum StellarNetwork {
    Testnet,
    Mainnet,
}

impl StellarNetwork {
    /// Returns the well-known public Horizon URL for this network.
    /// Used as the default when `HORIZON_URL` is not explicitly configured.
    pub fn default_horizon_url(&self) -> &'static str {
        match self {
            StellarNetwork::Testnet => "https://horizon-testnet.stellar.org",
            StellarNetwork::Mainnet => "https://horizon.stellar.org",
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            StellarNetwork::Testnet => "testnet",
            StellarNetwork::Mainnet => "mainnet",
        }
    }
}

impl Config {
    /// Build configuration from CLI flags and environment variables.
    ///
    /// `HORIZON_URL` is optional — when omitted it defaults to the well-known
    /// public Horizon endpoint for the selected `STELLAR_NETWORK`.
    pub fn from_sources(cli: &Cli) -> Result<Self, String> {
        Self::from_sources_with_env(cli, &std::collections::HashMap::new())
    }

    /// Like `from_sources` but allows injecting env var overrides — used in tests
    /// to avoid mutating the real process environment.
    #[cfg(test)]
    pub fn from_sources_with_overrides(
        cli: &Cli,
        overrides: &std::collections::HashMap<&str, &str>,
    ) -> Result<Self, String> {
        Self::from_sources_with_env(cli, &overrides.iter().map(|(k, v)| (*k, *v)).collect())
    }

    fn from_sources_with_env(
        cli: &Cli,
        overrides: &std::collections::HashMap<&str, &str>,
    ) -> Result<Self, String> {
        let get = |key: &str| -> Option<String> {
            overrides
                .get(key)
                .map(|v| v.to_string())
                .or_else(|| env::var(key).ok())
        };

        // -------- Network --------
        let network_raw = cli
            .network
            .clone()
            .or_else(|| get("STELLAR_NETWORK"))
            .ok_or("STELLAR_NETWORK is required")?;

        let stellar_network = match network_raw.as_str() {
            "testnet" => StellarNetwork::Testnet,
            "mainnet" => StellarNetwork::Mainnet,
            other => return Err(format!("Invalid STELLAR_NETWORK: {}", other)),
        };

        // -------- Horizon URL --------
        let horizon_url = cli
            .horizon_url
            .clone()
            .or_else(|| get("HORIZON_URL"))
            .unwrap_or_else(|| stellar_network.default_horizon_url().to_string());

        // -------- Poll Interval --------
        let poll_interval_seconds = cli
            .poll_interval
            .or_else(|| get("POLL_INTERVAL_SECONDS")?.parse().ok())
            .ok_or("POLL_INTERVAL_SECONDS is required and must be a number")?;
        // tokio::time::interval panics on a zero duration.
        if poll_interval_seconds == 0 {
            return Err("POLL_INTERVAL_SECONDS must be > 0".to_string());
        }

        // -------- API Port --------
        // API_PORT takes precedence; fall back to PORT (Render's injected var),
        // then 8080 for local development.
        let api_port = match get("API_PORT") {
            Some(raw) => parse_env_value("API_PORT", &raw)?,
            None => match get("PORT") {
                Some(raw) => parse_env_value("PORT", &raw)?,
                None => 8080,
            },
        };

        // -------- Cache TTL --------
        let cache_ttl_seconds = get("CACHE_TTL_SECONDS")
            .map(|raw| parse_env_value::<u64>("CACHE_TTL_SECONDS", &raw))
            .transpose()?
            .unwrap_or(5);

        // -------- API key --------
        let api_key = get("API_KEY").filter(|v| !v.trim().is_empty());

        // -------- Rate limiting --------
        let rate_limit_per_minute = get("RATE_LIMIT_PER_MINUTE")
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(60);

        // -------- Alerts --------
        let webhook_url = get("WEBHOOK_URL").filter(|v| !v.trim().is_empty());
        let alert_threshold = get("ALERT_THRESHOLD")
            .map(|v| parse_spike_severity(&v))
            .transpose()?
            .unwrap_or(SpikeSeverity::Major);

        let enabled_alert_types = match get("ENABLED_ALERT_TYPES") {
            Some(raw) => {
                let parsed: Vec<String> = raw
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                for alert_type in &parsed {
                    if !VALID_ALERT_TYPES.contains(&alert_type.as_str()) {
                        return Err(format!("Invalid ENABLED_ALERT_TYPES entry: {}", alert_type));
                    }
                }
                parsed
            }
            None => VALID_ALERT_TYPES.iter().map(|s| s.to_string()).collect(),
        };

        let stale_data_threshold_seconds = get("STALE_DATA_THRESHOLD_SECONDS")
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(300);

        // -------- Allowed Origins --------
        let allowed_origins = get("ALLOWED_ORIGINS")
            .unwrap_or_else(|| "http://localhost:3000".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // -------- Retry config --------
        let retry_attempts = get("RETRY_ATTEMPTS")
            .map(|raw| parse_env_value::<u32>("RETRY_ATTEMPTS", &raw))
            .transpose()?
            .unwrap_or(3);

        let base_retry_delay_ms = get("BASE_RETRY_DELAY_MS")
            .map(|raw| parse_env_value::<u64>("BASE_RETRY_DELAY_MS", &raw))
            .transpose()?
            .unwrap_or(1000);

        // -------- Database URL --------
        let database_url =
            get("DATABASE_URL").unwrap_or_else(|| "sqlite://stellar_fees.db".to_string());

        // -------- Storage retention --------
        let storage_retention_days = get("STORAGE_RETENTION_DAYS")
            .map(|raw| parse_env_value::<u64>("STORAGE_RETENTION_DAYS", &raw))
            .transpose()?
            .unwrap_or(7);

        // -------- Recommendation config --------
        let rec_defaults = RecommendationConfig::default();
        let recommendation = RecommendationConfig {
            default_confidence: get("REC_DEFAULT_CONFIDENCE")
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(rec_defaults.default_confidence),
            default_ledgers: get("REC_DEFAULT_LEDGERS")
                .and_then(|v| v.parse::<u8>().ok())
                .unwrap_or(rec_defaults.default_ledgers),
            history_window_secs: get("REC_HISTORY_WINDOW_SECS")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(rec_defaults.history_window_secs),
            cache_ttl_secs: get("REC_CACHE_TTL_SECS")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(rec_defaults.cache_ttl_secs),
            min_sample_count: get("REC_MIN_SAMPLE_COUNT")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(rec_defaults.min_sample_count),
        };

        Ok(Self {
            stellar_network,
            horizon_url,
            poll_interval_seconds,
            cache_ttl_seconds,
            api_key,
            rate_limit_per_minute,
            webhook_url,
            alert_threshold,
            api_port,
            allowed_origins,
            retry_attempts,
            base_retry_delay_ms,
            database_url,
            storage_retention_days,
            recommendation,
            enabled_alert_types,
            stale_data_threshold_seconds,
        })
    }
}

/// Parse an environment value, failing loudly (rather than silently falling
/// back to a default) when it is set but not a valid value for `T`.
fn parse_env_value<T: std::str::FromStr>(name: &str, raw: &str) -> Result<T, String> {
    raw.trim()
        .parse::<T>()
        .map_err(|_| format!("Invalid {}: '{}'", name, raw))
}

fn parse_spike_severity(value: &str) -> Result<SpikeSeverity, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "minor" => Ok(SpikeSeverity::Minor),
        "moderate" => Ok(SpikeSeverity::Moderate),
        "major" => Ok(SpikeSeverity::Major),
        "critical" => Ok(SpikeSeverity::Critical),
        _ => Err(format!("Invalid ALERT_THRESHOLD: {}", value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_cli(network: &str, horizon_url: Option<&str>) -> Cli {
        Cli {
            network: Some(network.to_string()),
            horizon_url: horizon_url.map(str::to_string),
            poll_interval: Some(30),
        }
    }

    fn no_env<'a>() -> HashMap<&'a str, &'a str> {
        HashMap::new()
    }

    // ---- StellarNetwork::default_horizon_url ----

    #[test]
    fn testnet_defaults_to_testnet_horizon() {
        assert_eq!(
            StellarNetwork::Testnet.default_horizon_url(),
            "https://horizon-testnet.stellar.org"
        );
    }

    #[test]
    fn mainnet_defaults_to_mainnet_horizon() {
        assert_eq!(
            StellarNetwork::Mainnet.default_horizon_url(),
            "https://horizon.stellar.org"
        );
    }

    // ---- Config::from_sources_with_overrides ----

    #[test]
    fn testnet_without_horizon_url_uses_default() {
        let cli = make_cli("testnet", None);
        let config = Config::from_sources_with_overrides(&cli, &no_env()).unwrap();
        assert_eq!(config.horizon_url, "https://horizon-testnet.stellar.org");
    }

    #[test]
    fn mainnet_without_horizon_url_uses_default() {
        let cli = make_cli("mainnet", None);
        let config = Config::from_sources_with_overrides(&cli, &no_env()).unwrap();
        assert_eq!(config.horizon_url, "https://horizon.stellar.org");
    }

    #[test]
    fn explicit_horizon_url_overrides_default() {
        let custom = "https://my-private-horizon.example.com";
        let cli = make_cli("testnet", Some(custom));
        let config = Config::from_sources_with_overrides(&cli, &no_env()).unwrap();
        assert_eq!(config.horizon_url, custom);
    }

    #[test]
    fn invalid_network_returns_error() {
        let cli = make_cli("devnet", None);
        let result = Config::from_sources_with_overrides(&cli, &no_env());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid STELLAR_NETWORK"));
    }

    #[test]
    fn api_port_defaults_to_8080() {
        let cli = make_cli("testnet", None);
        let config = Config::from_sources_with_overrides(&cli, &no_env()).unwrap();
        assert_eq!(config.api_port, 8080);
    }

    #[test]
    fn cache_ttl_defaults_to_five_seconds() {
        let cli = make_cli("testnet", None);
        let config = Config::from_sources_with_overrides(&cli, &no_env()).unwrap();
        assert_eq!(config.cache_ttl_seconds, 5);
    }

    #[test]
    fn cache_ttl_uses_env_override() {
        let cli = make_cli("testnet", None);
        let env = HashMap::from([("CACHE_TTL_SECONDS", "12")]);
        let config = Config::from_sources_with_overrides(&cli, &env).unwrap();
        assert_eq!(config.cache_ttl_seconds, 12);
    }

    #[test]
    fn api_key_defaults_to_none() {
        let cli = make_cli("testnet", None);
        let config = Config::from_sources_with_overrides(&cli, &no_env()).unwrap();
        assert_eq!(config.api_key, None);
    }

    #[test]
    fn rate_limit_defaults_to_sixty_requests_per_minute() {
        let cli = make_cli("testnet", None);
        let config = Config::from_sources_with_overrides(&cli, &no_env()).unwrap();
        assert_eq!(config.rate_limit_per_minute, 60);
    }

    #[test]
    fn rate_limit_uses_env_override() {
        let cli = make_cli("testnet", None);
        let env = HashMap::from([("RATE_LIMIT_PER_MINUTE", "120")]);
        let config = Config::from_sources_with_overrides(&cli, &env).unwrap();
        assert_eq!(config.rate_limit_per_minute, 120);
    }

    #[test]
    fn rate_limit_zero_falls_back_to_default() {
        let cli = make_cli("testnet", None);
        let env = HashMap::from([("RATE_LIMIT_PER_MINUTE", "0")]);
        let config = Config::from_sources_with_overrides(&cli, &env).unwrap();
        assert_eq!(config.rate_limit_per_minute, 60);
    }

    #[test]
    fn api_key_reads_from_env() {
        let cli = make_cli("testnet", None);
        let env = HashMap::from([("API_KEY", "secret")]);
        let config = Config::from_sources_with_overrides(&cli, &env).unwrap();
        assert_eq!(config.api_key.as_deref(), Some("secret"));
    }

    #[test]
    fn empty_api_key_is_treated_as_unset() {
        let cli = make_cli("testnet", None);
        let env = HashMap::from([("API_KEY", "   ")]);
        let config = Config::from_sources_with_overrides(&cli, &env).unwrap();
        assert!(config.api_key.is_none());
    }

    #[test]
    fn webhook_url_defaults_to_none() {
        let cli = make_cli("testnet", None);
        let config = Config::from_sources_with_overrides(&cli, &no_env()).unwrap();
        assert!(config.webhook_url.is_none());
    }

    #[test]
    fn webhook_url_uses_env_value() {
        let cli = make_cli("testnet", None);
        let env = HashMap::from([("WEBHOOK_URL", "https://example.com/hook")]);
        let config = Config::from_sources_with_overrides(&cli, &env).unwrap();
        assert_eq!(
            config.webhook_url.as_deref(),
            Some("https://example.com/hook")
        );
    }

    #[test]
    fn zero_poll_interval_is_rejected() {
        let mut cli = make_cli("testnet", None);
        cli.poll_interval = Some(0);
        let result = Config::from_sources_with_overrides(&cli, &no_env());
        assert!(result.unwrap_err().contains("POLL_INTERVAL_SECONDS must be > 0"));
    }

    #[test]
    fn unparsable_api_port_returns_error() {
        let cli = make_cli("testnet", None);
        let env = HashMap::from([("API_PORT", "abc")]);
        let result = Config::from_sources_with_overrides(&cli, &env);
        assert!(result.unwrap_err().contains("Invalid API_PORT"));
    }

    #[test]
    fn unparsable_retry_attempts_returns_error() {
        let cli = make_cli("testnet", None);
        let env = HashMap::from([("RETRY_ATTEMPTS", "many")]);
        let result = Config::from_sources_with_overrides(&cli, &env);
        assert!(result.unwrap_err().contains("Invalid RETRY_ATTEMPTS"));
    }

    #[test]
    fn alert_threshold_defaults_to_major() {
        let cli = make_cli("testnet", None);
        let config = Config::from_sources_with_overrides(&cli, &no_env()).unwrap();
        assert_eq!(config.alert_threshold, SpikeSeverity::Major);
    }

    #[test]
    fn alert_threshold_parses_from_env() {
        let cli = make_cli("testnet", None);
        let env = HashMap::from([("ALERT_THRESHOLD", "Critical")]);
        let config = Config::from_sources_with_overrides(&cli, &env).unwrap();
        assert_eq!(config.alert_threshold, SpikeSeverity::Critical);
    }

    #[test]
    fn invalid_alert_threshold_returns_error() {
        let cli = make_cli("testnet", None);
        let env = HashMap::from([("ALERT_THRESHOLD", "Severe")]);
        let result = Config::from_sources_with_overrides(&cli, &env);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid ALERT_THRESHOLD"));
    }

    #[test]
    fn allowed_origins_defaults_to_localhost_3000() {
        let cli = make_cli("testnet", None);
        let config = Config::from_sources_with_overrides(&cli, &no_env()).unwrap();
        assert_eq!(config.allowed_origins, vec!["http://localhost:3000"]);
    }

    #[test]
    fn allowed_origins_parses_comma_separated_list() {
        let cli = make_cli("testnet", None);
        let env = HashMap::from([(
            "ALLOWED_ORIGINS",
            "http://localhost:3000,https://app.example.com",
        )]);
        let config = Config::from_sources_with_overrides(&cli, &env).unwrap();
        assert_eq!(
            config.allowed_origins,
            vec!["http://localhost:3000", "https://app.example.com"]
        );
    }

    #[test]
    fn allowed_origins_trims_whitespace() {
        let cli = make_cli("testnet", None);
        let env = HashMap::from([(
            "ALLOWED_ORIGINS",
            "http://localhost:3000 , https://app.example.com ",
        )]);
        let config = Config::from_sources_with_overrides(&cli, &env).unwrap();
        assert_eq!(
            config.allowed_origins,
            vec!["http://localhost:3000", "https://app.example.com"]
        );
    }

    // ---- enabled_alert_types / stale_data_threshold_seconds (Issue #556) ----

    #[test]
    fn enabled_alert_types_defaults_to_all_four() {
        let cli = make_cli("testnet", None);
        let config = Config::from_sources_with_overrides(&cli, &no_env()).unwrap();
        let mut types = config.enabled_alert_types.clone();
        types.sort();
        assert_eq!(
            types,
            vec!["good_window", "recovery", "spike", "stale_data"]
        );
    }

    #[test]
    fn enabled_alert_types_parses_env_subset() {
        let cli = make_cli("testnet", None);
        let env = HashMap::from([("ENABLED_ALERT_TYPES", "spike,stale_data")]);
        let config = Config::from_sources_with_overrides(&cli, &env).unwrap();
        assert_eq!(config.enabled_alert_types, vec!["spike", "stale_data"]);
    }

    #[test]
    fn enabled_alert_types_rejects_invalid_entry() {
        let cli = make_cli("testnet", None);
        let env = HashMap::from([("ENABLED_ALERT_TYPES", "spike,made_up_type")]);
        let result = Config::from_sources_with_overrides(&cli, &env);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Invalid ENABLED_ALERT_TYPES entry"));
    }

    #[test]
    fn stale_data_threshold_defaults_to_300_seconds() {
        let cli = make_cli("testnet", None);
        let config = Config::from_sources_with_overrides(&cli, &no_env()).unwrap();
        assert_eq!(config.stale_data_threshold_seconds, 300);
    }

    #[test]
    fn stale_data_threshold_uses_env_override() {
        let cli = make_cli("testnet", None);
        let env = HashMap::from([("STALE_DATA_THRESHOLD_SECONDS", "600")]);
        let config = Config::from_sources_with_overrides(&cli, &env).unwrap();
        assert_eq!(config.stale_data_threshold_seconds, 600);
    }

    #[test]
    fn stale_data_threshold_zero_falls_back_to_default() {
        let cli = make_cli("testnet", None);
        let env = HashMap::from([("STALE_DATA_THRESHOLD_SECONDS", "0")]);
        let config = Config::from_sources_with_overrides(&cli, &env).unwrap();
        assert_eq!(config.stale_data_threshold_seconds, 300);
    }
}
