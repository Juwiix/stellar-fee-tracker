//! Demonstrates reusing a single reqwest::Client instead of
//! constructing a new one per request.
use once_cell::sync::Lazy;

// The build result is stored as-is. A failed build is reported to every caller of
// `shared_client` instead of panicking inside the initialiser.
static SHARED_CLIENT: Lazy<Result<reqwest::Client, reqwest::Error>> =
    Lazy::new(|| reqwest::Client::builder().build());

/// Returns the shared client, or the error that prevented it from being built.
///
/// `reqwest::Error` is not `Clone`, so the error is returned by reference to the
/// stored value; the build is attempted once and the outcome is reused.
pub fn shared_client() -> Result<&'static reqwest::Client, &'static reqwest::Error> {
    SHARED_CLIENT.as_ref()
}
