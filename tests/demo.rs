//! The demo (examples/demo): its config loads, and the routing it shows is what the rules do. The
//! files themselves are loaded, so the demo cannot drift from what the gateway does. No upstream is
//! called: decisions are checked with `explain`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test scaffolding

mod common;

use common::serve;
use humpyard::config::Config;
use humpyard::server;
use serde_json::{Value, json};

async fn gateway() -> (String, String) {
    let (key, hash) = humpyard::auth::generate_key().unwrap();
    let (_, other_a) = humpyard::auth::generate_key().unwrap();
    let (_, other_b) = humpyard::auth::generate_key().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let text = include_str!("../examples/demo/demo.toml")
        .replace("__LEDGER__", &dir.path().join("l.db").display().to_string())
        .replace("__TOUR_HASH__", &hash)
        .replace("__BEFORE_HASH__", &other_a)
        .replace("__AFTER_HASH__", &other_b);
    std::mem::forget(dir); // the ledger file must outlive the test body
    let config = Config::from_toml(&text, |_| Some("k".into())).unwrap();
    let url = format!(
        "http://{}",
        serve(server::router(config).await.unwrap()).await
    );
    (url, key)
}

async fn explain(url: &str, key: &str, body: Value) -> Value {
    reqwest::Client::new()
        .post(format!("{url}/v1/route/explain"))
        .bearer_auth(key)
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn the_demo_routes_each_kind_of_request_as_its_script_says() {
    let (url, key) = gateway().await;
    let cases = [
        (json!({"model": "auto"}), "cheap", "everyday"),
        (
            json!({"model": "auto", "tools": true}),
            "capable",
            "tool-turns",
        ),
        (
            json!({"model": "auto", "headers": {"x-humpyard-profile": "private"}}),
            "private-pool",
            "private-profile",
        ),
        (
            json!({"model": "auto", "prompt_tokens": 40_000}),
            "long-context",
            "long-prompt",
        ),
        // The "before" half of the comparison asks for the big model by name.
        (json!({"model": "capable"}), "capable", ""),
    ];
    for (request, route, rule) in cases {
        let report = explain(&url, &key, request.clone()).await;
        assert_eq!(report["selected"]["route"], route, "{request}: {report}");
        if !rule.is_empty() {
            assert_eq!(report["selected"]["rule"], rule, "{request}");
        }
    }
}
