//! Minimal Horizon HTTP client with a configurable base URL and timeout.
use std::time::Duration;

pub struct HorizonClient {
    base_url: String,
    timeout: Duration,
    client: reqwest::Client,
}

impl HorizonClient {
    /// Builds a client for `base_url` with the given request `timeout`.
    ///
    /// Returns the underlying [`reqwest::Error`] if the HTTP client cannot be
    /// built (for example when the TLS backend fails to initialise), so callers
    /// can report it instead of the process panicking.
    pub fn new(base_url: impl Into<String>, timeout: Duration) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder().timeout(timeout).build()?;

        Ok(Self { base_url: base_url.into(), timeout, client })
    }

    pub async fn fee_stats_raw(&self) -> Result<String, reqwest::Error> {
        let url = format!("{}/fee_stats", self.base_url);
        self.client.get(url).timeout(self.timeout).send().await?.text().await
    }
}
