//! Target classes (`private`, `public`, `paid`, ...) are labels for visibility: a fallback that
//! crosses classes is flagged on decisions and in `explain`. Nothing about routing changes.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test scaffolding

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use common::{chat_completion, chat_request, serve, usage};
use humpyard::config::Config;
use humpyard::server;
use serde_json::{Value, json};

struct Upstream(AtomicU16);

async fn completions(State(up): State<Arc<Upstream>>) -> axum::response::Response {
    match up.0.load(Ordering::SeqCst) {
        200 => axum::Json(chat_completion("m", "ok", Some(usage(10, 5)))).into_response(),
        code => (
            StatusCode::from_u16(code).unwrap(),
            axum::Json(json!({"error": {"message": "nope"}})),
        )
            .into_response(),
    }
}

async fn upstream(status: u16) -> (Arc<Upstream>, String) {
    let up = Arc::new(Upstream(AtomicU16::new(status)));
    let app = Router::new()
        .route("/chat/completions", post(completions))
        .with_state(up.clone());
    (up, format!("http://{}", serve(app).await))
}

struct Rig {
    url: String,
    key: String,
    _dir: tempfile::TempDir,
}

/// Targets: `priv-a` and `priv-b` (class private, failing), `pub` (class public, healthy), `plain`
/// (no class, failing). Routes: `crossing` (priv-a then pub), `same` (priv-a then priv-b then pub),
/// `unlabeled` (plain then pub), `solo` (pub).
async fn rig() -> Rig {
    let (_failing, bad) = upstream(503).await;
    let (_healthy, good) = upstream(200).await;
    let dir = tempfile::tempdir().unwrap();
    let (key, hash) = humpyard::auth::generate_key().unwrap();
    let toml = format!(
        r#"listen = "127.0.0.1:0"
[ledger]
path = "{db}"
[keys.k]
sha256 = "{hash}"
[providers.bad]
base_url = "{bad}"
api_key_env = "K"
max_retries = 0
[providers.good]
base_url = "{good}"
api_key_env = "K"
max_retries = 0
[targets.priv-a]
class = "private"
endpoints = [{{ provider = "bad", model = "a" }}]
[targets.priv-b]
class = "private"
endpoints = [{{ provider = "bad", model = "b" }}]
[targets.pub]
class = "public"
endpoints = [{{ provider = "good", model = "p" }}]
[targets.plain]
endpoints = [{{ provider = "bad", model = "x" }}]
[routes.crossing]
type = "passthrough"
targets = ["priv-a", "pub"]
[routes.same]
type = "passthrough"
targets = ["priv-a", "priv-b"]
[routes.unlabeled]
type = "passthrough"
targets = ["plain", "pub"]
[routes.solo]
type = "passthrough"
targets = ["pub"]
"#,
        db = dir.path().join("l.db").display()
    );
    let config = Config::from_toml(&toml, |_| Some("k".into())).unwrap();
    Rig {
        url: format!(
            "http://{}",
            serve(server::router(config).await.unwrap()).await
        ),
        key,
        _dir: dir,
    }
}

impl Rig {
    async fn ask(&self, model: &str) -> u16 {
        reqwest::Client::new()
            .post(format!("{}/v1/chat/completions", self.url))
            .bearer_auth(&self.key)
            .json(&chat_request(model, false))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    }

    async fn last_decision(&self) -> Value {
        let body: Value = reqwest::Client::new()
            .get(format!("{}/v1/decisions?limit=1", self.url))
            .bearer_auth(&self.key)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        body["data"][0].clone()
    }

    async fn explain(&self, model: &str) -> Value {
        reqwest::Client::new()
            .post(format!("{}/v1/route/explain", self.url))
            .bearer_auth(&self.key)
            .json(&json!({"model": model}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn a_private_target_failing_over_to_a_public_one_is_flagged_and_still_served() {
    let r = rig().await;
    assert_eq!(
        r.ask("crossing").await,
        200,
        "behavior is unchanged: the fallback still serves"
    );
    let d = r.last_decision().await;
    assert_eq!(d["fallback_crossed_classes"], true);
    let classes: Vec<(&str, &str)> = d["attempts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| (a["target"].as_str().unwrap(), a["class"].as_str().unwrap()))
        .collect();
    assert_eq!(classes, [("priv-a", "private"), ("pub", "public")]);
    assert_eq!(d["served"]["target"], "pub");
}

#[tokio::test]
async fn same_class_unlabeled_and_single_attempt_requests_are_not_flagged() {
    let r = rig().await;
    // Two private targets, both failing: no class was crossed (and the client gets the error).
    assert_eq!(r.ask("same").await, 502);
    assert_eq!(r.last_decision().await["fallback_crossed_classes"], false);
    // A target without a class: the answer is unknown, never a guess.
    assert_eq!(r.ask("unlabeled").await, 200);
    assert_eq!(
        r.last_decision().await["fallback_crossed_classes"],
        Value::Null
    );
    // One attempt on one classed target: not crossed.
    assert_eq!(r.ask("solo").await, 200);
    assert_eq!(r.last_decision().await["fallback_crossed_classes"], false);
}

#[tokio::test]
async fn explain_shows_target_classes_and_whether_falling_through_can_cross_them() {
    let r = rig().await;
    let crossing = r.explain("crossing").await;
    assert_eq!(crossing["classes"], json!(["private", "public"]));
    assert_eq!(crossing["fallback_can_cross_classes"], true);
    let targets = crossing["targets"].as_array().unwrap();
    assert_eq!(targets[0]["class"], "private");
    assert_eq!(targets[1]["class"], "public");

    let same = r.explain("same").await;
    assert_eq!(same["classes"], json!(["private"]));
    assert_eq!(same["fallback_can_cross_classes"], false);

    let solo = r.explain("solo").await;
    assert_eq!(solo["fallback_can_cross_classes"], false);
    // Unlabeled targets have no class; the route reports only what is known.
    let unlabeled = r.explain("unlabeled").await;
    assert_eq!(unlabeled["classes"], json!(["public"]));
    assert_eq!(unlabeled["targets"][0]["class"], Value::Null);
}

#[test]
fn class_names_are_validated_and_optional() {
    let load = |class: &str| {
        Config::from_toml(
            &format!(
                "listen = \"127.0.0.1:0\"\n[providers.p]\nbase_url = \"http://x\"\napi_key_env = \"K\"\n\
                 [targets.t]\n{class}\nendpoints = [{{ provider = \"p\", model = \"m\" }}]\n"
            ),
            |_| Some("k".into()),
        )
        .map_err(|e| e.to_string())
    };
    for ok in ["class = \"private\"", "class = \"free-tier_2\"", ""] {
        assert!(load(ok).is_ok(), "{ok}");
    }
    for bad in [
        "class = \"\"",
        "class = \"Private\"",
        "class = \"has space\"",
        &format!("class = \"{}\"", "x".repeat(33)),
    ] {
        let message = load(bad).err().unwrap_or_else(|| panic!("accepted {bad}"));
        assert!(message.contains("class"), "{bad}: {message}");
    }
    assert_eq!(
        load("class = \"paid\"").unwrap().target_classes["t"],
        "paid"
    );
    assert!(load("").unwrap().target_classes.is_empty());
}
