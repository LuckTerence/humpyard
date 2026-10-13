//! `GET /v1/quota`: the latest usage windows of every provider that names a quota source, with
//! the age of the data, or why it is unknown. Read-only; credentials never appear (ADR 0015).

use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};
use switchyard_protocol::WireFormat;

use super::{AppState, authenticate};
use crate::quota::{View, Window, identity};

fn window_json(window: &Window) -> Value {
    json!({
        "name": window.name,
        "percent": window.percent,
        "status": window.status,
        "resets_at": window.resets_at,
        "clamped": window.clamped,
    })
}

pub(super) async fn get_quota(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let snap = state.snapshot();
    if let Err(error) = authenticate(&snap, &headers).await {
        return error.into_response_for(WireFormat::OpenAiChat);
    }
    let config = &snap.config;
    let now = state.clock.now_ms();
    let stale_ms = i64::try_from(config.quota_stale_secs.saturating_mul(1000)).unwrap_or(i64::MAX);
    let providers: serde_json::Map<String, Value> = config
        .providers
        .iter()
        .filter_map(|(name, provider)| {
            let source = provider.quota_source.as_deref()?;
            let view = state
                .quota
                .view(name, &identity(source, &provider.base_url), now, stale_ms);
            let body = match view {
                View::Fresh {
                    windows,
                    fetched_ms,
                    age_ms,
                    last_error,
                } => json!({
                    "source": source,
                    "state": "fresh",
                    "fetched_at_ms": fetched_ms,
                    "age_secs": age_ms / 1000,
                    "windows": windows.iter().map(window_json).collect::<Vec<_>>(),
                    "last_error": last_error,
                }),
                View::Unknown { reason } => json!({
                    "source": source,
                    "state": "unknown",
                    "reason": reason,
                }),
            };
            Some((name.clone(), body))
        })
        .collect();
    axum::Json(json!({
        "object": "quota",
        "poll_secs": config.quota_poll_secs,
        "stale_secs": config.quota_stale_secs,
        "providers": providers,
    }))
    .into_response()
}
