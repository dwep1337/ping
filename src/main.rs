mod check;
mod discord;
mod state;

use check::{check_url, CheckResult};
use state::Status;
use std::env;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

fn require_env(name: &str) -> String {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => value.trim().to_string(),
        _ => {
            eprintln!("Missing required env var: {name}");
            process::exit(1);
        }
    }
}

fn parse_positive_u64(name: &str, fallback: u64) -> u64 {
    match env::var(name) {
        Ok(raw) => match raw.parse::<u64>() {
            Ok(value) if value > 0 => value,
            _ => {
                eprintln!("Invalid {name}: expected a positive integer");
                process::exit(1);
            }
        },
        Err(_) => fallback,
    }
}

fn parse_mention_ids(name: &str) -> Vec<String> {
    match env::var(name) {
        Ok(raw) => {
            let ids: Vec<String> = raw
                .split(',')
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(str::to_string)
                .collect();
            if ids.is_empty() {
                eprintln!("Invalid {name}: expected comma-separated Discord user IDs");
                process::exit(1);
            }
            ids
        }
        Err(_) => {
            eprintln!("Missing required env var: {name}");
            process::exit(1);
        }
    }
}

static RUNNING: AtomicBool = AtomicBool::new(false);

async fn run_check(url: &str, webhook: &str, mention_ids: &[String], timeout_ms: u64) {
    if RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        eprintln!("Previous check still running, skipping");
        return;
    }

    let result = async {
        let result = check_url(url, timeout_ms).await;
        let next = if result.is_ok() {
            Status::Up
        } else {
            Status::Down
        };
        let previous = state::get_last_status();
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        match &result {
            CheckResult::Ok { status } => {
                println!("[{now}] UP ({status}) {url}");
            }
            CheckResult::Err { reason } => {
                println!("[{now}] DOWN {url} — {reason}");
            }
        }

        // First check: alert only if already down (skip "recovered" on boot)
        if previous.is_none() {
            if let CheckResult::Err { reason } = &result {
                if next == Status::Down {
                    discord::notify(webhook, url, mention_ids, discord::Alert::Down { reason })
                        .await?;
                    println!("Discord: notified down");
                }
            }
            state::set_last_status(next);
            return Ok::<(), String>(());
        }

        if previous == Some(next) {
            return Ok(());
        }

        match &result {
            CheckResult::Err { reason } if next == Status::Down => {
                discord::notify(webhook, url, mention_ids, discord::Alert::Down { reason }).await?;
                println!("Discord: notified down");
            }
            CheckResult::Ok { .. } if next == Status::Up => {
                discord::notify(webhook, url, mention_ids, discord::Alert::Up).await?;
                println!("Discord: notified recovered");
            }
            _ => {}
        }

        state::set_last_status(next);
        Ok(())
    }
    .await;

    if let Err(error) = result {
        eprintln!("Check cycle failed: {error}");
    }

    RUNNING.store(false, Ordering::SeqCst);
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    let url = require_env("URL");
    let webhook = require_env("WEB_HOOK");
    let mention_ids = parse_mention_ids("MENTION_USER_IDS");
    let interval_ms = parse_positive_u64("INTERVAL_MS", 60_000);
    let timeout_ms = parse_positive_u64("TIMEOUT_MS", 10_000);

    println!("Monitoring {url} every {interval_ms}ms (timeout {timeout_ms}ms)");

    run_check(&url, &webhook, &mention_ids, timeout_ms).await;

    let mut ticker = tokio::time::interval(Duration::from_millis(interval_ms));
    ticker.tick().await; // consume the immediate first tick
    loop {
        ticker.tick().await;
        run_check(&url, &webhook, &mention_ids, timeout_ms).await;
    }
}
