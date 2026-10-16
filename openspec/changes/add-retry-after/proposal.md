# Proposal

## Why

A rate-limited free endpoint tells the caller when it will serve again. humpyard cools a failing endpoint for a
guessed time (10 seconds, doubling to a cap of 35, after `failure_threshold` consecutive failures), so it either
keeps sending requests into a 429 or comes back too early. ModelRelay honors the provider's retry time; this is
step 1 of "Replacing ModelRelay".

Switchyard's HTTP client reads `Retry-After` but, in 0.3.0, does not put it on the error it returns
(`UpstreamHttp { status, body }`), so the header cannot be read by humpyard yet. Upstream PR
NVIDIA-NeMo/Switchyard#904 (closing #903) adds the headers to that error. Until a release has it, many
providers repeat the same information in the error body ("Please try again in 7.66s", or a `retryDelay` field),
which humpyard can read now.

## What Changes

- **A retry time is read from a failed attempt**: from the upstream error body now (an interim), and from the
  `Retry-After` header once Switchyard exposes it (a follow-up that changes one function). The body is read for
  a small set of well-known phrasings and fields only.
- **A rate-limited endpoint (429, or 503) with a retry time is cooled for that long**, immediately, without
  waiting for `failure_threshold` failures, and without the doubling. When the cooldown ends, one request probes
  it as before.
- **Bounded**: the retry time is clamped to at least 1 second and at most `[health] max_retry_after_secs`
  (default 300); anything unreadable or out of range is ignored, and the endpoint behaves exactly as today.
- **Visible**: endpoint health shows the cooldown's source (`retry_after` or `failures`), decisions and
  `route/explain` already show a cooling target as skipped, and a counter records how often a retry time was
  honored and where it came from.
- **Out of scope:** reading `x-ratelimit-reset-*` or per-minute quota headers, predicting limits before a 429,
  changing Switchyard's own internal retries (`max_retries`), and any change to routes or fallback order.

## Capabilities

### Modified Capabilities
- `routing-policy`: endpoint health and cooldown.

## Impact

`src/health.rs` (a cooldown from a given duration), `src/pool.rs` (read the hint on a failed attempt), a small
parser module (`src/retry_after.rs`), `src/config` (`max_retry_after_secs`), `src/metrics.rs`, `src/server.rs`
(health snapshot). Docs: `routing.md` (Endpoint health), `configuration.md`, `metrics.md`.
