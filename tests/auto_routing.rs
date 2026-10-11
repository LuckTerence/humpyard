//! The auto-routing recipe (examples/auto-routing.toml): one client-facing `auto` model whose
//! selector rules pick the pool by intent. The example file itself is loaded, so the documented
//! recipe cannot drift from what works. Decisions are checked with `explain`; no upstream is called.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test scaffolding

mod common;

use common::serve;
use humpyard::config::Config;
use humpyard::server;
use serde_json::{Value, json};

struct Rig {
    url: String,
    full: String,
    auto_only: String,
}

/// The example config plus two keys: `full` (no restriction) and `auto_only` (may use `auto` and
/// the pools the rules choose, but not `private-pool`).
async fn rig() -> Rig {
    let (full, full_hash) = humpyard::auth::generate_key().unwrap();
    let (auto_only, auto_hash) = humpyard::auth::generate_key().unwrap();
    let text = format!(
        "{}\n[keys.full]\nsha256 = \"{full_hash}\"\n[keys.limited]\nsha256 = \"{auto_hash}\"\n\
         allowed_routes = [\"auto\", \"cheap\", \"capable\", \"long-context\", \"vision\"]\n",
        include_str!("../examples/auto-routing.toml")
            .replace("humpyard.db", "auto-routing-test.db")
    );
    let dir = tempfile::tempdir().unwrap();
    let text = text.replace(
        "auto-routing-test.db",
        &dir.path().join("l.db").display().to_string(),
    );
    std::mem::forget(dir); // the ledger file must outlive the test body
    let config = Config::from_toml(&text, |_| Some("k".into())).unwrap();
    Rig {
        url: format!(
            "http://{}",
            serve(server::router(config).await.unwrap()).await
        ),
        full,
        auto_only,
    }
}

impl Rig {
    async fn explain(&self, key: &str, body: Value) -> Value {
        let key = if key == "full" {
            &self.full
        } else {
            &self.auto_only
        };
        reqwest::Client::new()
            .post(format!("{}/v1/route/explain", self.url))
            .bearer_auth(key)
            .json(&body)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }
}

/// (rule that fires, route chosen) for a hypothetical request.
fn chosen(report: &Value) -> (&str, &str) {
    (
        report["selected"]["rule"].as_str().unwrap(),
        report["selected"]["route"].as_str().unwrap(),
    )
}

#[tokio::test]
async fn each_intent_rule_picks_its_pool() {
    let r = rig().await;
    let cases = [
        (
            json!({"model": "auto", "headers": {"x-humpyard-profile": "private"}}),
            ("private-profile", "private-pool"),
        ),
        (
            json!({"model": "auto", "headers": {"x-humpyard-tag-privacy": "private"}}),
            ("private-tag", "private-pool"),
        ),
        (
            json!({"model": "auto", "images": true}),
            ("images", "vision"),
        ),
        (
            json!({"model": "auto", "prompt_tokens": 45_000}),
            ("long-prompt", "long-context"),
        ),
        (
            json!({"model": "auto", "subagent": true}),
            ("subagent", "cheap"),
        ),
        (
            json!({"model": "auto", "tools": true}),
            ("tool-turns", "capable"),
        ),
        (json!({"model": "auto"}), ("everyday", "cheap")),
    ];
    for (body, expected) in cases {
        let report = r.explain("full", body.clone()).await;
        assert_eq!(report["outcome"], "ok", "{body}: {report}");
        assert_eq!(chosen(&report), expected, "{body}");
        assert_eq!(report["selected"]["source"], "selector");
    }
}

#[tokio::test]
async fn rule_order_decides_when_several_apply() {
    let r = rig().await;
    // Privacy beats everything: a private request with images, tools and a huge prompt stays private.
    let all = json!({"model": "auto", "images": true, "tools": true, "prompt_tokens": 90_000, "subagent": true,
                     "headers": {"x-humpyard-profile": "private"}});
    assert_eq!(
        chosen(&r.explain("full", all).await),
        ("private-profile", "private-pool")
    );
    // Images beat long prompts, long prompts beat sub-agent and tools.
    let img_long = json!({"model": "auto", "images": true, "prompt_tokens": 90_000});
    assert_eq!(
        chosen(&r.explain("full", img_long).await),
        ("images", "vision")
    );
    let long_tools =
        json!({"model": "auto", "tools": true, "subagent": true, "prompt_tokens": 40_000});
    assert_eq!(
        chosen(&r.explain("full", long_tools).await),
        ("long-prompt", "long-context")
    );
    // A sub-agent turn with tools stays cheap (the sub-agent rule is listed first).
    let sub_tools = json!({"model": "auto", "tools": true, "subagent": true});
    assert_eq!(
        chosen(&r.explain("full", sub_tools).await),
        ("subagent", "cheap")
    );
    // The long-prompt threshold is inclusive.
    let edge = json!({"model": "auto", "prompt_tokens": 30_000});
    assert_eq!(chosen(&r.explain("full", edge).await).0, "long-prompt");
    let below = json!({"model": "auto", "prompt_tokens": 29_999});
    assert_eq!(chosen(&r.explain("full", below).await).0, "everyday");
}

#[tokio::test]
async fn the_rules_only_touch_auto_never_a_model_the_client_named() {
    let r = rig().await;
    // Asking for a pool directly bypasses the rules even when every condition would match.
    let direct = r
        .explain("full", json!({"model": "capable", "images": true, "headers": {"x-humpyard-profile": "private"}}))
        .await;
    assert_eq!(
        direct["selected"],
        json!({"route": "capable", "rule": "default", "source": "requested_model"})
    );
    assert_eq!(direct["outcome"], "ok");
}

#[tokio::test]
async fn a_key_that_may_not_use_the_private_pool_is_never_sent_there_by_a_header() {
    let r = rig().await;
    // The limited key asks for privacy, but its allowlist excludes `private-pool`: the privacy
    // rules are skipped (clients narrow, never widen) and the request falls to the next rule.
    let report = r
        .explain(
            "limited",
            json!({"model": "auto", "headers": {"x-humpyard-profile": "private"}}),
        )
        .await;
    assert_eq!(chosen(&report), ("everyday", "cheap"));
    let skipped: Vec<&str> = report["rules"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|rule| rule["matched"] == true && rule["key_may_use_route"] == false)
        .map(|rule| rule["rule"].as_str().unwrap())
        .collect();
    assert_eq!(
        skipped,
        ["private-profile"],
        "the skip is visible in explain"
    );
    // Everything else works for the limited key, so `auto` is usable without private access.
    assert_eq!(
        chosen(
            &r.explain("limited", json!({"model": "auto", "tools": true}))
                .await
        ),
        ("tool-turns", "capable")
    );
}

#[tokio::test]
async fn auto_is_safe_when_every_rule_is_skipped() {
    let r = rig().await;
    // A key allowed only `auto`: every rule's route is off limits, so `auto` itself serves.
    let (key, hash) = humpyard::auth::generate_key().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let text = format!(
        "{}\n[keys.only-auto]\nsha256 = \"{hash}\"\nallowed_routes = [\"auto\"]\n",
        include_str!("../examples/auto-routing.toml").replace(
            "humpyard.db",
            &dir.path().join("o.db").display().to_string()
        )
    );
    let config = Config::from_toml(&text, |_| Some("k".into())).unwrap();
    let url = format!(
        "http://{}",
        serve(server::router(config).await.unwrap()).await
    );
    let report: Value = reqwest::Client::new()
        .post(format!("{url}/v1/route/explain"))
        .bearer_auth(&key)
        .json(&json!({"model": "auto", "tools": true, "images": true}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(report["outcome"], "ok", "{report}");
    assert_eq!(
        report["selected"],
        json!({"route": "auto", "rule": "default", "source": "requested_model"})
    );
    let _ = r;
}

#[tokio::test]
async fn a_private_request_never_lists_a_public_target() {
    let r = rig().await;
    let report = r
        .explain(
            "full",
            json!({"model": "auto", "headers": {"x-humpyard-profile": "private"}}),
        )
        .await;
    let targets: Vec<&str> = report["targets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["target"].as_str().unwrap())
        .collect();
    assert_eq!(
        targets,
        ["local-model"],
        "the private pool has no public fallback in this recipe"
    );
}
