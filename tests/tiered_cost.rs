//! Tiered prices end to end: a call's cost in the ledger uses the tier its prompt size falls into.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::format_push_string
)] // test scaffolding

mod common;

use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::post;
use common::{chat_completion, chat_request, serve, usage};
use humpyard::config::Config;
use humpyard::ledger::Ledger;
use humpyard::server;
use serde_json::Value;

/// Answers with a prompt size taken from the model name: `p<N>-m` reports N prompt tokens.
async fn completions(
    _: State<()>,
    axum::Json(body): axum::Json<Value>,
) -> axum::response::Response {
    let model = body["model"].as_str().unwrap();
    let prompt: u64 = model
        .trim_start_matches('p')
        .trim_end_matches("-m")
        .parse()
        .unwrap();
    axum::Json(chat_completion(model, "ok", Some(usage(prompt, 1_000)))).into_response()
}

#[tokio::test]
async fn cost_in_the_ledger_follows_the_prompt_size_tier() {
    let upstream = serve(
        Router::new()
            .route("/chat/completions", post(completions))
            .with_state(()),
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("l.db");
    let (key, hash) = humpyard::auth::generate_key().unwrap();
    // $1/$4 per million up to 100,000 prompt tokens, $3/$12 above.
    let price = "{ input = 1.0, output = 4.0, tiers = [{ above_tokens = 100000, input = 3.0, output = 12.0 }] }";
    let mut toml = format!(
        "listen = \"127.0.0.1:0\"\n[ledger]\npath = \"{}\"\n[keys.k]\nsha256 = \"{hash}\"\n[providers.p]\nbase_url = \"http://{upstream}\"\napi_key_env = \"K\"\n",
        db.display()
    );
    for prompt in [1_000u64, 100_000, 100_001, 150_000] {
        toml.push_str(&format!(
            "[targets.t{prompt}]\nendpoints = [{{ provider = \"p\", model = \"p{prompt}-m\", price = {price} }}]\n"
        ));
    }
    let config = Config::from_toml(&toml, |_| Some("k".into())).unwrap();
    let url = format!(
        "http://{}",
        serve(server::router(config).await.unwrap()).await
    );
    for prompt in [1_000u64, 100_000, 100_001, 150_000] {
        let resp = reqwest::Client::new()
            .post(format!("{url}/v1/chat/completions"))
            .bearer_auth(&key)
            .json(&chat_request(&format!("t{prompt}"), false))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }
    // The ledger writes asynchronously: wait for all four rows.
    let mut rows = Vec::new();
    for _ in 0..100 {
        rows = Ledger::open(&db)
            .await
            .unwrap()
            .entries_since(0)
            .await
            .unwrap();
        if rows.len() == 4 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(rows.len(), 4);
    let cost = |model: &str| {
        rows.iter()
            .find(|r| r.model == model)
            .unwrap()
            .cost_micro_usd
    };
    assert_eq!(cost("p1000-m"), 1_000 + 4_000, "small prompt: base price");
    assert_eq!(
        cost("p100000-m"),
        100_000 + 4_000,
        "exactly at the threshold: still base"
    );
    assert_eq!(
        cost("p100001-m"),
        100_001 * 3 + 1_000 * 12,
        "one token over: tier price"
    );
    assert_eq!(cost("p150000-m"), 150_000 * 3 + 1_000 * 12);
}
