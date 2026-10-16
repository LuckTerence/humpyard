//! Session id sources: a session id sent as `session_id` or `x-session-affinity` (what `OpenClaw` sends with
//! session affinity on) lands on the usage rows and in the session views, and an unusable value is served
//! without a session.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test scaffolding

mod common;

use axum::Router;
use axum::response::IntoResponse;
use axum::routing::post;
use common::{chat_completion, chat_request, serve, usage};
use humpyard::config::Config;
use humpyard::server;
use serde_json::Value;

async fn completions() -> axum::response::Response {
    axum::Json(chat_completion("m", "ok", Some(usage(10, 5)))).into_response()
}

async fn gateway() -> (String, String, tempfile::TempDir) {
    let upstream = serve(Router::new().route("/chat/completions", post(completions))).await;
    let dir = tempfile::tempdir().unwrap();
    let (key, hash) = humpyard::auth::generate_key().unwrap();
    let toml = format!(
        r#"listen = "127.0.0.1:0"
[ledger]
path = "{db}"
[keys.alice]
sha256 = "{hash}"
[providers.p]
base_url = "http://{upstream}"
api_key_env = "K"
max_retries = 0
[targets.t]
endpoints = [{{ provider = "p", model = "m", price = {{ input = 1.0, output = 2.0 }} }}]
[routes.plain]
type = "passthrough"
targets = ["t"]
"#,
        db = dir.path().join("ledger.db").display()
    );
    let config = Config::from_toml(&toml, |_| Some("k".into())).unwrap();
    let url = format!(
        "http://{}",
        serve(server::router(config).await.unwrap()).await
    );
    (url, key, dir)
}

async fn chat(url: &str, key: &str, header: Option<(&str, &str)>) {
    let mut request = reqwest::Client::new()
        .post(format!("{url}/v1/chat/completions"))
        .bearer_auth(key)
        .json(&chat_request("plain", false));
    if let Some((name, value)) = header {
        request = request.header(name, value);
    }
    let response = request.send().await.unwrap();
    assert_eq!(response.status(), 200, "the request is served either way");
}

#[tokio::test]
async fn fallback_headers_make_sessions_and_unusable_ones_do_not() {
    let (url, key, _dir) = gateway().await;
    chat(&url, &key, Some(("session_id", "oc-1"))).await;
    chat(&url, &key, Some(("session_id", "oc-1"))).await;
    chat(&url, &key, Some(("x-session-affinity", "oc-2"))).await;
    chat(&url, &key, Some(("x-switchyard-session-id", "sy-1"))).await;
    // Recognized header wins over the fallback on the same request.
    chat(&url, &key, Some(("x-switchyard-session-id", "sy-1"))).await;
    chat(&url, &key, Some(("session_id", "has a space"))).await;
    chat(&url, &key, Some(("x-client-request-id", "req-9"))).await;
    chat(&url, &key, None).await;

    let body: Value = reqwest::Client::new()
        .get(format!("{url}/v1/stats?group_by=session&window=1h"))
        .bearer_auth(&key)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["sessions_total"], 3, "{body}");
    assert_eq!(body["answered_without_session"], 3, "{body}");
    let calls = |name: &str| {
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["group"] == name)
            .unwrap_or_else(|| panic!("no session {name} in {body}"))["requests"]
            .clone()
    };
    assert_eq!(calls("oc-1"), 2);
    assert_eq!(calls("oc-2"), 1);
    assert_eq!(calls("sy-1"), 2);

    // The decision log records the same id.
    let decisions: Value = reqwest::Client::new()
        .get(format!("{url}/v1/decisions?limit=20"))
        .bearer_auth(&key)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ids: Vec<&str> = decisions["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| d["session_id"].as_str())
        .collect();
    assert!(ids.contains(&"oc-1") && ids.contains(&"oc-2"), "{ids:?}");
    assert!(!ids.contains(&"has a space"), "{ids:?}");
}
