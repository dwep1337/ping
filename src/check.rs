use reqwest::StatusCode;
use std::time::Duration;

pub enum CheckResult {
    Ok { status: u16 },
    Err { reason: String },
}

impl CheckResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok { .. })
    }
}

/// Cloudflare HTTP erros
fn is_origin_down(status: StatusCode) -> bool {
    matches!(
        status.as_u16(),
        502 | 503 | 504 | 521 | 522 | 523 | 524 | 525 | 526
    )
}

pub async fn check_url(url: &str, timeout_ms: u64) -> CheckResult {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return CheckResult::Err {
                reason: error.to_string(),
            };
        }
    };

    match client.get(url).send().await {
        Ok(response) => {
            let status = response.status();
            if is_origin_down(status) {
                CheckResult::Err {
                    reason: format!("HTTP {} {}", status.as_u16(), status.canonical_reason().unwrap_or("")).trim().to_string(),
                }
            } else {
                CheckResult::Ok {
                    status: status.as_u16(),
                }
            }
        }
        Err(error) => {
            let reason = if error.is_timeout() {
                format!("Timeout after {timeout_ms}ms")
            } else {
                error.without_url().to_string()
            };
            CheckResult::Err { reason }
        }
    }
}
