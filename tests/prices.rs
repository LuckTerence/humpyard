//! Price sources: parsing untrusted data, comparing it with a config, the fetch limits, and the
//! `humpyard prices` commands run as a real process. Nothing here touches the real internet.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test scaffolding

mod common;

use axum::Router;
use axum::http::StatusCode;
use axum::routing::get;
use common::serve;
use humpyard::config::Config;
use humpyard::prices::{self, Status};

const FIXTURE: &str = include_str!("fixtures/models-dev-sample.json");

fn data() -> prices::SourceData {
    prices::parse_models_dev(FIXTURE, "opencode-go").unwrap()
}

fn config(extra_targets: &str) -> Config {
    Config::from_toml(
        &format!(
            r#"listen = "127.0.0.1:0"
[providers.opencode]
base_url = "http://x"
api_key_env = "K"
[providers.other]
base_url = "http://y"
api_key_env = "K"
{extra_targets}"#
        ),
        |_| Some("k".into()),
    )
    .unwrap()
}

const TARGETS: &str = r#"
[targets.same]
endpoints = [{ provider = "opencode", model = "glm-5.3-flash", price = { input = 0.15, output = 0.5, cached_input = 0.03 } }]
[targets.changed]
endpoints = [{ provider = "opencode", model = "kimi-k3", price = { input = 3.0, output = 12.0 } }]
[targets.unpriced]
endpoints = [{ provider = "opencode", model = "free-model" }]
[targets.gone]
endpoints = [{ provider = "opencode", model = "retired-model", price = { input = 1.0, output = 1.0 } }]
[targets.broken]
endpoints = [{ provider = "opencode", model = "negative", price = { input = 1.0, output = 2.0 } }]
[targets.elsewhere]
endpoints = [{ provider = "other", model = "glm-5.3-flash", price = { input = 9.0, output = 9.0 } }]
"#;

#[test]
fn parses_flat_cached_free_and_tiered_prices() {
    let d = data();
    assert_eq!(d.source, "models-dev:opencode-go");
    let flat = &d.records["glm-5.3-flash"];
    assert_eq!(
        (flat.input, flat.output, flat.cached_input),
        (0.15, 0.5, Some(0.03))
    );
    assert_eq!(flat.tiers, Vec::new());
    let free = &d.records["free-model"];
    assert_eq!(
        (free.input, free.output, free.cached_input),
        (0.0, 0.0, None)
    );
}

#[test]
fn explicit_tiers_win_over_the_legacy_field_and_are_sorted() {
    let tiers = &data().records["minimax-m3"].tiers;
    let thresholds: Vec<u64> = tiers.iter().map(|t| t.above_tokens).collect();
    assert_eq!(
        thresholds,
        [256_000, 512_000],
        "sorted by threshold; legacy field ignored"
    );
    assert_eq!(
        (tiers[1].input, tiers[1].output, tiers[1].cached_input),
        (0.6, 2.4, Some(0.12))
    );
    let legacy = &data().records["legacy-tier"].tiers;
    assert_eq!(legacy.len(), 1);
    assert_eq!(legacy[0].above_tokens, 200_000);
    assert_eq!((legacy[0].input, legacy[0].output), (2.0, 4.0));
}

#[test]
fn a_tier_kind_it_cannot_model_is_noted_not_guessed() {
    let record = &data().records["deepseek-peak"];
    assert_eq!(
        (record.input, record.output),
        (0.15, 0.6),
        "the base price is kept"
    );
    assert_eq!(record.tiers, Vec::new());
    assert!(
        record.notes[0].contains("`time` tier"),
        "{:?}",
        record.notes
    );
}

#[test]
fn malformed_prices_are_reported_and_never_defaulted() {
    let d = data();
    for bad in ["negative", "absurd", "stringy", "no-output", "no-cost"] {
        assert!(!d.records.contains_key(bad), "{bad} must not be priced");
        assert!(d.unusable.contains_key(bad), "{bad} must be reported");
    }
    assert!(d.unusable["negative"].contains("`input`"));
    assert!(d.unusable["absurd"].contains("10000"));
    assert!(d.unusable["no-output"].contains("`output` is missing"));
    assert_eq!(d.unusable["no-cost"], "the source gives no price");
}

#[test]
fn a_bad_document_is_an_error_not_an_empty_result() {
    assert!(prices::parse_models_dev("not json", "opencode-go").is_err());
    let missing = prices::parse_models_dev(FIXTURE, "nope")
        .unwrap_err()
        .to_string();
    assert!(missing.contains("no provider `nope`"), "{missing}");
    let no_models = prices::parse_models_dev(r#"{"p": {"id": "p"}}"#, "p").unwrap_err();
    assert!(no_models.to_string().contains("lists no models"));
}

#[test]
fn source_specs_are_explicit() {
    assert_eq!(
        prices::parse_source("models-dev:opencode-go").unwrap(),
        "opencode-go"
    );
    for bad in ["opencode-go", "models-dev:", "other:x", ""] {
        assert!(prices::parse_source(bad).is_err(), "{bad}");
    }
}

#[test]
fn compare_classifies_every_endpoint_of_the_named_provider_only() {
    let rows = prices::compare(&config(TARGETS), "opencode", &data());
    let status: std::collections::BTreeMap<_, _> = rows
        .iter()
        .map(|r| (r.target.as_str(), r.status.clone()))
        .collect();
    assert_eq!(status["same"], Status::Ok);
    assert_eq!(status["changed"], Status::Differs);
    assert_eq!(status["unpriced"], Status::Unpriced);
    assert_eq!(status["gone"], Status::MissingAtSource);
    assert!(matches!(&status["broken"], Status::Unusable(r) if r.contains("`input`")));
    assert!(
        !status.contains_key("elsewhere"),
        "other providers are not compared"
    );
    assert!(prices::has_drift(&rows));
}

#[test]
fn matching_ignores_float_noise_and_an_unset_cached_price_equals_the_input_price() {
    let cfg = config(
        r#"[targets.t]
endpoints = [
  { provider = "opencode", model = "glm-5.3-flash", price = { input = 0.15000000001, output = 0.5, cached_input = 0.03 } },
  { provider = "opencode", model = "legacy-tier", price = { input = 1.0, output = 2.0 } },
]"#,
    );
    let rows = prices::compare(&cfg, "opencode", &data());
    assert!(rows.iter().all(|r| r.status == Status::Ok), "{rows:?}");
    // The source charges a cheaper cache read than the config's implied one: that is drift.
    let drifted = config(
        r#"[targets.t]
endpoints = [{ provider = "opencode", model = "glm-5.3-flash", price = { input = 0.15, output = 0.5 } }]"#,
    );
    let rows = prices::compare(&drifted, "opencode", &data());
    assert_eq!(rows[0].status, Status::Differs);
    // Without a source cache price there is nothing to disagree about.
    let uncached = config(
        r#"[targets.t]
endpoints = [{ provider = "opencode", model = "legacy-tier", price = { input = 1.0, output = 2.0, cached_input = 0.5 } }]"#,
    );
    let rows = prices::compare(&uncached, "opencode", &data());
    assert_eq!(rows[0].status, Status::Ok);
    // A cache price that differs from the source's is drift.
    let wrong_cache = config(
        r#"[targets.t]
endpoints = [{ provider = "opencode", model = "kimi-k3", price = { input = 3.0, output = 15.0, cached_input = 1.0 } }]"#,
    );
    assert_eq!(
        prices::compare(&wrong_cache, "opencode", &data())[0].status,
        Status::Differs
    );
}

#[test]
fn reports_name_what_changed_and_what_was_read() {
    let rows = prices::compare(&config(TARGETS), "opencode", &data());
    let report = prices::render_report(&rows, "models-dev:opencode-go", "2026-10-12");
    assert!(report.contains("read 2026-10-12"));
    assert!(report.contains("differs"));
    assert!(report.contains("configured: input = 3.0, output = 12.0"));
    assert!(report.contains("source:     input = 3.0, output = 15.0, cached_input = 0.3"));
    assert!(report.contains("missing-at-source"));
    assert!(report.contains("5 endpoint(s) checked, 4 need attention"));
    let json = prices::render_json(&rows, "models-dev:opencode-go", "2026-10-12");
    assert_eq!(json["drift"], true);
    let gone = json["endpoints"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["target"] == "gone")
        .unwrap();
    assert_eq!(gone["status"], "missing-at-source");
}

#[test]
fn the_snippet_is_for_review_and_carries_provenance_and_tier_notes() {
    let cfg = config(
        r#"[targets.m3]
endpoints = [{ provider = "opencode", model = "minimax-m3" }]
[targets.gone]
endpoints = [{ provider = "opencode", model = "retired-model" }]"#,
    );
    let rows = prices::compare(&cfg, "opencode", &data());
    let snippet = prices::render_snippet(&rows, "models-dev:opencode-go", "2026-10-12");
    assert!(snippet.starts_with("# Proposed prices from models-dev:opencode-go, read 2026-10-12."));
    assert!(snippet.contains("price = { input = 0.3, output = 1.2, cached_input = 0.06 }"));
    assert!(
        snippet.contains("above 256000 prompt tokens"),
        "tiers are surfaced, not applied"
    );
    assert!(snippet.contains("NOT PROPOSED: the source does not list this model"));
    // What it prints must itself be valid TOML a person can paste.
    let line = snippet.lines().find(|l| l.starts_with("price = ")).unwrap();
    let parsed: toml::Table = toml::from_str(line).unwrap();
    assert_eq!(parsed["price"]["input"].as_float(), Some(0.3));
}

#[tokio::test]
async fn only_https_or_loopback_http_is_fetched_and_failures_are_errors() {
    for url in [
        "http://example.com/api.json",
        "ftp://models.dev/api.json",
        "file:///etc/passwd",
    ] {
        let error = prices::fetch(url).await.unwrap_err().to_string();
        assert!(error.contains("https"), "{url}: {error}");
    }
    let app = Router::new()
        .route("/ok", get(|| async { FIXTURE }))
        .route("/missing", get(|| async { StatusCode::NOT_FOUND }))
        .route(
            "/moved",
            get(|| async { (StatusCode::FOUND, [("location", "http://example.com/x")]) }),
        );
    let base = format!("http://{}", serve(app).await);
    assert!(
        prices::fetch(&format!("{base}/ok"))
            .await
            .unwrap()
            .contains("opencode-go")
    );
    let missing = prices::fetch(&format!("{base}/missing"))
        .await
        .unwrap_err()
        .to_string();
    assert!(missing.contains("404"), "{missing}");
    let moved = prices::fetch(&format!("{base}/moved"))
        .await
        .unwrap_err()
        .to_string();
    assert!(moved.contains("302"), "redirects are not followed: {moved}");
}

// ---- the commands, as real processes ----

struct Cli {
    url: String,
    dir: tempfile::TempDir,
}

async fn cli() -> Cli {
    let app = Router::new().route("/api.json", get(|| async { FIXTURE }));
    Cli {
        url: format!("http://{}/api.json", serve(app).await),
        dir: tempfile::tempdir().unwrap(),
    }
}

impl Cli {
    fn write_config(&self, targets: &str) -> std::path::PathBuf {
        let path = self.dir.path().join("config.toml");
        let text = format!(
            "listen = \"127.0.0.1:0\"\n[providers.opencode]\nbase_url = \"http://x\"\napi_key_env = \"HUMPYARD_TEST_NO_SUCH_KEY\"\n{targets}"
        );
        std::fs::write(&path, text).unwrap();
        path
    }

    async fn run(&self, args: &[&str]) -> (i32, String, String) {
        let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_humpyard"))
            .arg("prices")
            .args(args)
            .env_remove("HUMPYARD_TEST_NO_SUCH_KEY")
            .output()
            .await
            .unwrap();
        (
            out.status.code().unwrap(),
            String::from_utf8(out.stdout).unwrap(),
            String::from_utf8(out.stderr).unwrap(),
        )
    }
}

const GOOD: &str = r#"
[targets.a]
endpoints = [{ provider = "opencode", model = "glm-5.3-flash", price = { input = 0.15, output = 0.5, cached_input = 0.03 } }]
[targets.b]
endpoints = [{ provider = "opencode", model = "kimi-k3", price = { input = 3.0, output = 15.0, cached_input = 0.3 } }]
"#;

#[tokio::test]
async fn check_exits_zero_without_drift_and_one_with_it() {
    let c = cli().await;
    let url = c.url.clone();
    let args = |config: &std::path::Path| -> Vec<String> {
        [
            "check",
            "--config",
            config.to_str().unwrap(),
            "--source",
            "models-dev:opencode-go",
            "--provider",
            "opencode",
            "--url",
            &url,
        ]
        .map(String::from)
        .to_vec()
    };
    let run = |args: Vec<String>| {
        let c = &c;
        async move {
            c.run(&args.iter().map(String::as_str).collect::<Vec<_>>())
                .await
        }
    };
    let (code, out, _) = run(args(&c.write_config(GOOD))).await;
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("2 endpoint(s) checked, 0 need attention"));

    let drifted = c.write_config(&GOOD.replace("output = 15.0", "output = 12.0"));
    let (code, out, _) = run(args(&drifted)).await;
    assert_eq!(code, 1, "drift is exit 1: {out}");
    assert!(out.contains("differs"));

    let mut json = args(&drifted);
    json.push("--json".into());
    let (code, out, _) = c
        .run(&json.iter().map(String::as_str).collect::<Vec<_>>())
        .await;
    assert_eq!(code, 1);
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed["drift"], true);
    assert_eq!(parsed["endpoints"][1]["status"], "differs");
}

#[tokio::test]
async fn import_prints_a_proposal_and_never_touches_the_config() {
    let c = cli().await;
    let path = c.write_config(GOOD);
    let before = std::fs::read(&path).unwrap();
    let (code, out, _) = c
        .run(&[
            "import",
            "--config",
            path.to_str().unwrap(),
            "--source",
            "models-dev:opencode-go",
            "--provider",
            "opencode",
            "--url",
            &c.url,
        ])
        .await;
    assert_eq!(code, 0);
    assert!(
        out.contains("price = { input = 3.0, output = 15.0, cached_input = 0.3 }"),
        "{out}"
    );
    assert!(out.contains("nothing was changed"));
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "the config file is untouched"
    );
}

#[tokio::test]
async fn unusable_sources_and_configs_exit_two_with_a_reason() {
    let c = cli().await;
    let path = c.write_config(GOOD);
    let p = path.to_str().unwrap();
    let cases: [(Vec<&str>, &str); 5] = [
        (
            vec![
                "--source",
                "nope:x",
                "--provider",
                "opencode",
                "--url",
                &c.url,
            ],
            "unknown price source",
        ),
        (
            vec![
                "--source",
                "models-dev:opencode-go",
                "--provider",
                "ghost",
                "--url",
                &c.url,
            ],
            "no provider `ghost`",
        ),
        (
            vec![
                "--source",
                "models-dev:nonexistent",
                "--provider",
                "opencode",
                "--url",
                &c.url,
            ],
            "no provider `nonexistent`",
        ),
        (
            vec![
                "--source",
                "models-dev:opencode-go",
                "--provider",
                "opencode",
                "--url",
                "http://127.0.0.1:1/api.json",
            ],
            "cannot reach",
        ),
        (
            vec![
                "--source",
                "models-dev:opencode-go",
                "--provider",
                "opencode",
                "--url",
                "http://example.com/x",
            ],
            "https",
        ),
    ];
    for (extra, expected) in cases {
        let mut args = vec!["check", "--config", p];
        args.extend(extra);
        let (code, out, err) = c.run(&args).await;
        assert_eq!(code, 2, "{expected}: {out}{err}");
        assert!(err.contains(expected), "{expected}: {err}");
        assert!(out.is_empty(), "no report on a failure: {out}");
    }
}
