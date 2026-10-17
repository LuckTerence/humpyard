//! The generated targets file (ADR 0022), `serve` side: a strict loader that accepts the whole file or
//! ignores it, never fetches anything, never lets a generated target claim a class, never makes a
//! configuration invalid, and is picked up by the reload poll.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test scaffolding

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::post;
use common::{chat_completion, serve, usage};
use humpyard::config::{Config, DiscoveryState};
use humpyard::server::{self, Reload};
use serde_json::{Value, json};
use tokio::sync::mpsc;

/// One metrics credential for the whole file: its key and the hash the config carries.
fn credential() -> &'static (String, String) {
    static CREDENTIAL: std::sync::OnceLock<(String, String)> = std::sync::OnceLock::new();
    CREDENTIAL.get_or_init(|| humpyard::auth::generate_key().unwrap())
}

/// A main config with provider `free` opted in and `lab` not, a hand-written private target `home`, and a
/// pool route over the `general` strength. `file` is the generated file's path.
fn main_config(url: &str, file: &Path, extra: &str) -> String {
    format!(
        r#"listen = "127.0.0.1:0"
reload_poll_secs = 1
[metrics]
enabled = true
bearer_sha256 = "{hash}"
[discovery]
enabled = true
file = "{file}"
{extra}
[providers.free]
base_url = "{url}"
api_key_env = "PATH"
price_source = "models-dev:groq"
discover = {{ strengths = ["general"] }}
[providers.lab]
base_url = "{url}"
api_key_env = "PATH"
[targets.home]
class = "private"
strengths = ["general"]
endpoints = [{{ provider = "lab", model = "home-model" }}]
[routes.pool]
type = "fastest"
pool = {{ strength = "general" }}
[routes.private-pool]
type = "passthrough"
pool = {{ strength = "general" }}
allowed_classes = ["private"]
"#,
        file = file.display(),
        hash = credential().1
    )
}

const GOOD: &str = r#"
[targets.auto-free-alpha]
strengths = ["general", "all"]
endpoints = [{ provider = "free", model = "alpha", context_window = 32000, price = { input = 0.0, output = 0.0 } }]
[targets.auto-free-beta]
strengths = ["general", "all", "reasoning"]
endpoints = [{ provider = "free", model = "beta", price = { input = 0.0, output = 0.0 } }]
"#;

fn dir_file(name: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(name);
    (dir, file)
}

fn load(file_text: Option<&str>, extra: &str) -> (Config, tempfile::TempDir) {
    let (dir, file) = dir_file("discovered.toml");
    if let Some(text) = file_text {
        std::fs::write(&file, text).unwrap();
    }
    let main = main_config("http://127.0.0.1:1", &file, extra);
    let config = Config::from_toml_with(&main, |n| std::env::var(n).ok(), &|p| {
        std::fs::read_to_string(p).ok()
    })
    .unwrap();
    (config, dir)
}

#[test]
fn a_valid_generated_file_is_merged_and_joins_pools_by_strength() {
    let (config, _dir) = load(Some(GOOD), "");
    assert_eq!(config.discovery_state, DiscoveryState::Loaded(2));
    assert_eq!(
        config.generated_targets.iter().collect::<Vec<_>>(),
        ["auto-free-alpha", "auto-free-beta"]
    );
    assert_eq!(
        config.pools["pool"].members,
        ["auto-free-alpha", "auto-free-beta", "home"]
    );
    // The class always comes from the main config, never from the file.
    assert_eq!(config.target_classes["auto-free-alpha"], "public");
    assert!(!config.target_classes.contains_key("auto-nothing"));
}

#[test]
fn missing_empty_and_unconfigured_files_change_nothing() {
    let (config, _dir) = load(None, "");
    assert_eq!(config.discovery_state, DiscoveryState::Missing);
    assert_eq!(config.pools["pool"].members, ["home"]);
    let (config, _dir) = load(Some("  \n"), "");
    assert_eq!(config.discovery_state, DiscoveryState::Empty);
    let plain = Config::from_toml(
        r#"listen = "127.0.0.1:0"
[providers.p]
base_url = "http://127.0.0.1:1"
api_key_env = "PATH"
[targets.t]
endpoints = [{ provider = "p", model = "m" }]
"#,
        |n| std::env::var(n).ok(),
    )
    .unwrap();
    assert_eq!(plain.discovery_state, DiscoveryState::Off);
}

fn ignored(text: &str) -> String {
    let (config, _dir) = load(Some(text), "");
    match config.discovery_state {
        DiscoveryState::Ignored(reason) => {
            assert_eq!(
                config.pools["pool"].members,
                ["home"],
                "an ignored file adds nothing"
            );
            assert!(config.generated_targets.is_empty());
            reason
        }
        other => panic!("expected the file to be ignored, got {other:?} for:\n{text}"),
    }
}

#[test]
fn the_whole_file_is_ignored_when_any_rule_is_broken() {
    let free = "price = { input = 0.0, output = 0.0 }";
    // Only [targets.auto-*] tables are allowed.
    assert!(ignored(&format!("[keys.evil]\nsha256 = \"0\"\n{GOOD}")).contains("not a generated"));
    assert!(
        ignored(&format!("[providers.x]\nbase_url = \"http://x\"\n{GOOD}"))
            .contains("not a generated")
    );
    // Names, collisions, providers.
    assert!(
        ignored(&format!(
            "[targets.sneaky]\nendpoints = [{{ provider = \"free\", model = \"s\", {free} }}]\n"
        ))
        .contains("not named `auto-*`")
    );
    assert!(
        ignored(&format!(
            "[targets.auto-x]\nendpoints = [{{ provider = \"lab\", model = \"x\", {free} }}]\n"
        ))
        .contains("not opted in")
    );
    assert!(
        ignored(&format!(
            "[targets.auto-x]\nendpoints = [{{ provider = \"nobody\", model = \"x\", {free} }}]\n"
        ))
        .contains("not opted in")
    );
    assert!(ignored("[targets.auto-x]\nendpoints = []\n").contains("no endpoints"));
    // A (provider, model) pair that a hand-written target already uses, or one used twice.
    let (dir, file) = dir_file("d.toml");
    std::fs::write(
        &file,
        format!("[targets.auto-dup]\nendpoints = [{{ provider = \"free\", model = \"mine\", {free} }}]\n"),
    )
    .unwrap();
    let mut main = main_config("http://127.0.0.1:1", &file, "");
    main.push_str("[targets.mine]\nendpoints = [{ provider = \"free\", model = \"mine\" }]\n");
    let config = Config::from_toml_with(&main, |n| std::env::var(n).ok(), &|p| {
        std::fs::read_to_string(p).ok()
    })
    .unwrap();
    assert!(
        matches!(&config.discovery_state, DiscoveryState::Ignored(r) if r.contains("hand-written"))
    );
    drop(dir);
    assert!(
        ignored(&format!(
            "[targets.auto-a]\nendpoints = [{{ provider = \"free\", model = \"same\", {free} }}]\n\
         [targets.auto-b]\nendpoints = [{{ provider = \"free\", model = \"same\", {free} }}]\n"
        ))
        .contains("more than one generated target")
    );
}

#[test]
fn generated_targets_must_be_free_and_cannot_claim_a_class() {
    // Priced above zero, or not priced at all, is not a free model.
    assert!(ignored(
        "[targets.auto-paid]\nendpoints = [{ provider = \"free\", model = \"p\", price = { input = 0.5, output = 1.0 } }]\n"
    )
    .contains("not priced at zero"));
    assert!(
        ignored("[targets.auto-unpriced]\nendpoints = [{ provider = \"free\", model = \"p\" }]\n")
            .contains("not priced at zero")
    );
    assert!(ignored(
        "[targets.auto-tier]\nendpoints = [{ provider = \"free\", model = \"p\", price = { input = 0.0, output = 0.0, cached_input = 0.1 } }]\n"
    )
    .contains("not priced at zero"));
    // A generated target claiming to be private would reach strict private routes: refused.
    let reason = ignored(
        "[targets.auto-claim]\nclass = \"private\"\nendpoints = [{ provider = \"free\", model = \"p\", price = { input = 0.0, output = 0.0 } }]\n",
    );
    assert!(reason.contains("claims a class other than"), "{reason}");
    // Restating the configured class is fine.
    let ok = "[targets.auto-same]\nclass = \"public\"\nstrengths = [\"general\"]\nendpoints = [{ provider = \"free\", model = \"p\", price = { input = 0.0, output = 0.0 } }]\n";
    let (config, _dir) = load(Some(ok), "");
    assert_eq!(config.discovery_state, DiscoveryState::Loaded(1));
}

#[test]
fn strengths_and_size_are_checked_too() {
    assert!(ignored(
        "[targets.auto-bad]\nstrengths = [\"Not Valid\"]\nendpoints = [{ provider = \"free\", model = \"p\", price = { input = 0.0, output = 0.0 } }]\n"
    )
    .contains("strength"));
    let mut many = String::new();
    for i in 0..501 {
        use std::fmt::Write as _;
        write!(
            many,
            "[targets.auto-m{i}]\nendpoints = [{{ provider = \"free\", model = \"m{i}\", price = {{ input = 0.0, output = 0.0 }} }}]\n"
        )
        .unwrap();
    }
    assert!(ignored(&many).contains("more than the 500"));
    assert!(ignored("this is not toml {{").contains("not a generated"));
}

#[test]
fn a_generated_file_can_never_make_the_configuration_fail_to_load() {
    // A zero context window is rejected by the config rules; merged, it would fail the whole load.
    let bad = "[targets.auto-zero]\nendpoints = [{ provider = \"free\", model = \"z\", context_window = 0, price = { input = 0.0, output = 0.0 } }]\n";
    let (config, _dir) = load(Some(bad), "");
    match &config.discovery_state {
        DiscoveryState::Ignored(reason) => {
            assert!(
                reason.contains("made the configuration invalid"),
                "{reason}"
            );
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        config.pools["pool"].members,
        ["home"],
        "the main configuration loads alone"
    );
}

#[test]
fn the_discovery_settings_are_validated() {
    let (_dir, file) = dir_file("d.toml");
    let check = |extra: &str, tail: &str| {
        Config::from_toml(
            &format!("{}{tail}", main_config("http://127.0.0.1:1", &file, extra)),
            |n| std::env::var(n).ok(),
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    };
    assert!(
        check(
            "max_added_per_run = 3\nmax_removed_fraction = 0.25\nclass = \"free\"",
            ""
        )
        .is_ok()
    );
    assert!(
        check("max_added_per_run = 0", "")
            .unwrap_err()
            .contains("max_added_per_run")
    );
    assert!(
        check("max_removed_fraction = 1.5", "")
            .unwrap_err()
            .contains("max_removed_fraction")
    );
    assert!(check("class = \"\"", "").unwrap_err().contains("class"));
    assert!(check("exclude = [\"free/whisper\"]", "").is_ok());
    assert!(
        check("exclude = [\"nobody/whisper\"]", "")
            .unwrap_err()
            .contains("exclude")
    );
    assert!(
        check("exclude = [\"whisper\"]", "")
            .unwrap_err()
            .contains("exclude")
    );
    // A hand-written name in the generated namespace would silently disable the file: refused.
    let reserved = check(
        "",
        "[targets.auto-mine]\nendpoints = [{ provider = \"lab\", model = \"m\" }]\n",
    );
    assert!(reserved.unwrap_err().contains("reserved"));
    // Discovery needs the models.dev provider named.
    let no_source = Config::from_toml(
        r#"listen = "127.0.0.1:0"
[providers.p]
base_url = "http://127.0.0.1:1"
api_key_env = "PATH"
discover = { strengths = ["general"] }
[targets.t]
endpoints = [{ provider = "p", model = "m" }]
"#,
        |n| std::env::var(n).ok(),
    );
    assert!(no_source.unwrap_err().to_string().contains("price_source"));
}

#[test]
fn the_master_switch_is_off_by_default_and_turns_the_file_off() {
    // With the switch off the file is not even read: the targets are not merged and the state says so.
    let (_dir, file) = dir_file("discovered.toml");
    std::fs::write(&file, GOOD).unwrap();
    let main = main_config("http://127.0.0.1:1", &file, "").replace("enabled = true\n", "");
    let config = Config::from_toml_with(&main, |n| std::env::var(n).ok(), &|p| {
        std::fs::read_to_string(p).ok()
    })
    .unwrap();
    assert_eq!(config.discovery_state, DiscoveryState::Disabled);
    assert!(config.generated_targets.is_empty());
    assert_eq!(config.pools["pool"].members, ["home"]);
    // And an explicit false is the same.
    let off =
        main_config("http://127.0.0.1:1", &file, "").replace("enabled = true", "enabled = false");
    let config = Config::from_toml_with(&off, |n| std::env::var(n).ok(), &|p| {
        std::fs::read_to_string(p).ok()
    })
    .unwrap();
    assert_eq!(config.discovery_state, DiscoveryState::Disabled);
}

#[test]
fn enabling_discovery_needs_a_file() {
    let error = Config::from_toml(
        r#"listen = "127.0.0.1:0"
[discovery]
enabled = true
[providers.p]
base_url = "http://127.0.0.1:1"
api_key_env = "PATH"
[targets.t]
endpoints = [{ provider = "p", model = "m" }]
"#,
        |n| std::env::var(n).ok(),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("file"), "{error}");
}

#[test]
fn a_provider_can_opt_out_without_losing_its_settings() {
    // Two opted-in providers; the second switches itself off. Its targets are skipped at once and the
    // first provider's are still loaded: opting out one provider does not invalidate the file.
    let (_dir, file) = dir_file("discovered.toml");
    std::fs::write(
        &file,
        format!(
            "{GOOD}\n[targets.auto-two-gamma]\nstrengths = [\"general\"]\nendpoints = [{{ provider = \"two\", model = \"gamma\", price = {{ input = 0.0, output = 0.0 }} }}]\n"
        ),
    )
    .unwrap();
    let both = format!(
        "{}[providers.two]\nbase_url = \"http://127.0.0.1:1\"\napi_key_env = \"PATH\"\nprice_source = \"models-dev:groq\"\ndiscover = {{ strengths = [\"general\"] }}\n",
        main_config("http://127.0.0.1:1", &file, "")
    );
    let load = |text: &str| {
        Config::from_toml_with(text, |n| std::env::var(n).ok(), &|p| {
            std::fs::read_to_string(p).ok()
        })
        .unwrap()
    };
    assert_eq!(load(&both).discovery_state, DiscoveryState::Loaded(3));
    // Turn off only provider `two`.
    let opted_out = both.replacen(
        "[providers.two]\nbase_url = \"http://127.0.0.1:1\"\napi_key_env = \"PATH\"\nprice_source = \"models-dev:groq\"\ndiscover = { strengths = [\"general\"] }",
        "[providers.two]\nbase_url = \"http://127.0.0.1:1\"\napi_key_env = \"PATH\"\nprice_source = \"models-dev:groq\"\ndiscover = { strengths = [\"general\"], enabled = false }",
        1,
    );
    let config = load(&opted_out);
    assert_eq!(config.discovery_state, DiscoveryState::Loaded(2));
    assert!(!config.generated_targets.contains("auto-two-gamma"));
    assert!(config.generated_targets.contains("auto-free-alpha"));
}

struct Mock;

async fn completions(State(_): State<Arc<Mock>>) -> axum::response::Response {
    axum::Json(chat_completion("m", "ok", Some(usage(10, 5)))).into_response()
}

async fn mock() -> String {
    let app = Router::new()
        .route("/chat/completions", post(completions))
        .with_state(Arc::new(Mock));
    format!("http://{}", serve(app).await)
}

async fn members(base: &str, route: &str) -> Value {
    let explained: Value = reqwest::Client::new()
        .post(format!("{base}/v1/route/explain"))
        .json(&json!({"model": route}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    explained
}

async fn wait_members(base: &str, expected: Value) {
    for _ in 0..80 {
        if members(base, "pool").await["pool"]["members"] == expected {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!(
        "members never became {expected}: {}",
        members(base, "pool").await["pool"]
    );
}

async fn metrics(base: &str) -> String {
    reqwest::Client::new()
        .get(format!("{base}/metrics"))
        .bearer_auth(&credential().0)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap()
}

#[tokio::test]
async fn the_reload_poll_picks_up_a_changed_generated_file_and_fails_safe() {
    let url = mock().await;
    let (_dir, file) = dir_file("discovered.toml");
    let main = main_config(&url, &file, "");
    let main_path = file.parent().unwrap().join("humpyard.toml");
    std::fs::write(&main_path, &main).unwrap();
    let config = Config::load(&main_path).unwrap();
    assert_eq!(
        config.discovery_state,
        DiscoveryState::Missing,
        "no sync has run yet"
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (_reload, triggers) = mpsc::channel(4);
    let (_stop, signals) = mpsc::channel(4);
    tokio::spawn(server::run(
        config,
        listener,
        signals,
        Some(Reload {
            path: main_path.clone(),
            triggers,
        }),
    ));
    wait_members(&base, json!(["home"])).await;
    assert!(
        metrics(&base)
            .await
            .contains(r#"humpyard_discovery_file_state{state="missing"} 1"#)
    );

    // The first sync writes the file: only that file changed, and the gateway reloads on its own.
    std::fs::write(&file, GOOD).unwrap();
    wait_members(&base, json!(["auto-free-alpha", "auto-free-beta", "home"])).await;
    let explained = members(&base, "pool").await;
    let generated: Vec<(&str, bool)> = explained["targets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["target"].as_str().unwrap(),
                t["generated"].as_bool().unwrap(),
            )
        })
        .collect();
    assert!(
        generated.contains(&("auto-free-alpha", true)),
        "{generated:?}"
    );
    assert!(generated.contains(&("home", false)), "{generated:?}");
    let text = metrics(&base).await;
    assert!(text.contains("humpyard_discovery_targets 2"), "{text}");
    assert!(
        text.contains(r#"humpyard_discovery_file_state{state="loaded"} 1"#),
        "{text}"
    );

    // A later sync removes a model: the pool shrinks.
    std::fs::write(
        &file,
        GOOD.split("[targets.auto-free-beta]").next().unwrap(),
    )
    .unwrap();
    wait_members(&base, json!(["auto-free-alpha", "home"])).await;

    // A broken file is ignored in full and the gateway keeps serving the main configuration.
    std::fs::write(&file, "[keys.evil]\nsha256 = \"0\"\n").unwrap();
    wait_members(&base, json!(["home"])).await;
    assert!(
        metrics(&base)
            .await
            .contains(r#"humpyard_discovery_file_state{state="ignored"} 1"#)
    );
    let resp = reqwest::Client::new()
        .post(format!("{base}/v1/chat/completions"))
        .json(&common::chat_request("pool", false))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "requests are still served");
}

#[tokio::test]
async fn a_generated_target_is_never_admitted_to_a_strict_private_route() {
    let url = mock().await;
    let (_dir, file) = dir_file("discovered.toml");
    std::fs::write(&file, GOOD).unwrap();
    let config = Config::from_toml_with(
        &main_config(&url, &file, ""),
        |n| std::env::var(n).ok(),
        &|p| std::fs::read_to_string(p).ok(),
    )
    .unwrap();
    let base = format!(
        "http://{}",
        serve(server::router(config).await.unwrap()).await
    );
    for _ in 0..8 {
        let resp = reqwest::Client::new()
            .post(format!("{base}/v1/chat/completions"))
            .json(&common::chat_request("private-pool", false))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        assert_eq!(
            resp.headers()["x-humpyard-target"],
            "home",
            "the public generated targets are removed on a private-only route"
        );
    }
}
