//! Gateway overhead harness (spec: `performance`). Not part of the default run:
//!
//! ```text
//! cargo test --release --test overhead -- --ignored --nocapture
//! ```
//!
//! Sends the same request straight to a mock upstream and through the gateway, at several concurrencies and with
//! and without a fixed upstream delay, and reports the latency percentiles, the difference and requests per second.
//! Everything runs in one process on loopback, so it measures the gateway's own work and not a network.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test scaffolding
#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)] // reporting only

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::post;
use common::{chat_completion, chat_request, serve, usage};
use humpyard::config::Config;
use humpyard::server;

struct Mock {
    delay: Duration,
}

async fn completions(State(mock): State<Arc<Mock>>) -> axum::response::Response {
    if !mock.delay.is_zero() {
        tokio::time::sleep(mock.delay).await;
    }
    axum::Json(chat_completion("m", "ok", Some(usage(20, 5)))).into_response()
}

async fn mock_upstream(delay: Duration) -> String {
    let app = Router::new()
        .route("/chat/completions", post(completions))
        .with_state(Arc::new(Mock { delay }));
    format!("http://{}", serve(app).await)
}

async fn gateway(upstream: &str, dir: &std::path::Path) -> String {
    let toml = format!(
        r#"listen = "127.0.0.1:0"
[ledger]
path = "{}"
[providers.p]
base_url = "{upstream}"
api_key_env = "PATH"
max_retries = 0
[targets.t]
endpoints = [{{ provider = "p", model = "m", price = {{ input = 0.0, output = 0.0 }} }}]
[routes.r]
type = "passthrough"
targets = ["t"]
"#,
        dir.join("ledger.db").display()
    );
    let config = Config::from_toml(&toml, |n| std::env::var(n).ok()).unwrap();
    format!(
        "http://{}",
        serve(server::router(config).await.unwrap()).await
    )
}

struct Run {
    p50: f64,
    p90: f64,
    p99: f64,
    rps: f64,
}

/// The `percent`th percentile (nearest rank) of an ascending list.
fn percentile(sorted: &[f64], percent: usize) -> f64 {
    let index = ((sorted.len() - 1) * percent + 50) / 100;
    sorted[index.min(sorted.len() - 1)]
}

/// `requests` requests spread over `concurrency` workers; per-request latency in milliseconds.
async fn run(client: &reqwest::Client, url: &str, concurrency: usize, requests: usize) -> Run {
    let body = chat_request("r", false);
    let per_worker = requests.div_ceil(concurrency);
    let started = Instant::now();
    let workers: Vec<_> = (0..concurrency)
        .map(|_| {
            let (client, url, body) = (client.clone(), url.to_string(), body.clone());
            tokio::spawn(async move {
                let mut latencies = Vec::with_capacity(per_worker);
                for _ in 0..per_worker {
                    let t = Instant::now();
                    let resp = client.post(&url).json(&body).send().await.unwrap();
                    assert_eq!(resp.status(), 200);
                    let _ = resp.bytes().await.unwrap();
                    latencies.push(t.elapsed().as_secs_f64() * 1000.0);
                }
                latencies
            })
        })
        .collect();
    let mut all = Vec::new();
    for worker in workers {
        all.extend(worker.await.unwrap());
    }
    let elapsed = started.elapsed().as_secs_f64();
    all.sort_by(f64::total_cmp);
    Run {
        p50: percentile(&all, 50),
        p90: percentile(&all, 90),
        p99: percentile(&all, 99),
        rps: all.len() as f64 / elapsed,
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "overhead harness; run explicitly with --ignored"]
async fn gateway_overhead() {
    let dir = std::env::temp_dir().join(format!("humpyard-overhead-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(256)
        .build()
        .unwrap();
    let base_requests: usize = std::env::var("OVERHEAD_REQUESTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5_000);

    println!("\n=== gateway overhead: direct to a mock upstream versus through the gateway ===");
    println!(
        "{:<12}{:>5}  {:<8}{:>9}{:>9}{:>9}{:>11}   overhead p50 / p99 (ms)",
        "upstream", "conc", "path", "p50 ms", "p90 ms", "p99 ms", "req/s"
    );
    for delay_ms in [0u64, 50] {
        let delay = Duration::from_millis(delay_ms);
        let mock = mock_upstream(delay).await;
        std::fs::create_dir_all(dir.join(format!("d{delay_ms}"))).unwrap();
        let gw = gateway(&mock, &dir.join(format!("d{delay_ms}"))).await;
        let (direct_url, gateway_url) = (
            format!("{mock}/chat/completions"),
            format!("{gw}/v1/chat/completions"),
        );
        // Warm both paths (connections, caches, the ledger writer).
        run(&client, &direct_url, 8, 300).await;
        run(&client, &gateway_url, 8, 300).await;
        for concurrency in [1usize, 16, 64] {
            // A delayed upstream makes each request slow, so fewer are needed for the same precision.
            let requests = if delay_ms == 0 {
                base_requests
            } else {
                100 * concurrency
            };
            let direct = run(&client, &direct_url, concurrency, requests).await;
            let through = run(&client, &gateway_url, concurrency, requests).await;
            let label = if delay_ms == 0 {
                "instant".to_string()
            } else {
                format!("{delay_ms} ms")
            };
            for (path, r) in [("direct", &direct), ("gateway", &through)] {
                let note = if path == "gateway" {
                    format!(
                        "   +{:.2} / +{:.2}",
                        through.p50 - direct.p50,
                        through.p99 - direct.p99
                    )
                } else {
                    String::new()
                };
                println!(
                    "{label:<12}{concurrency:>5}  {path:<8}{:>9.2}{:>9.2}{:>9.2}{:>11.0}{note}",
                    r.p50, r.p90, r.p99, r.rps
                );
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
