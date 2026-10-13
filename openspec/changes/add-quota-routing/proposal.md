# Proposal

## Why

Flat-fee and capped plans (for example OpenCode Go: a monthly fee with dollar caps in a 5-hour, a
weekly and a monthly window) are the cheap way to run agents, and they fail in a way routing cannot
see today: a request goes to the provider, the provider answers 429 once a cap is hit, and humpyard
fails over. Health-aware routing (circuit breakers) limits the damage after repeated failures, but
it is reactive, and it cannot *pace* a quota: spending the weekly allowance by Tuesday on easy
sub-agent turns leaves nothing for the hard ones. Some providers publish the answer: OpenCode's
`GET /zen/go/v1/usage` returns, for the account, each window's `percent` used and when it resets.
Routing can use that, the way a database optimizer uses statistics about a scarce resource.

Measured on 2026-10-13 against a real account: the endpoint is **account-level** (it ignores a
`model` parameter and returns one set of three windows), so it says how hot the *account* is, not
which model is near its own cap.

## What Changes

- **A provider quota snapshot.** A provider may name a `quota_source` (first: `opencode-go`). While
  serving, humpyard polls it on a timer using the provider's own API key, only against the
  provider's own host, and keeps the latest snapshot: per window its percent used, status and reset
  time, plus when it was read. Polling is read-only, never on the request path, and a failure only
  means the snapshot goes stale; stale or missing data is *unknown*.
- **Quota as a selector condition.** `when = { quota = { provider, window, at_least | below } }`
  matches on a window's percent used, so an operator writes the pacing policy as ordinary rules, for
  example: when the weekly window is past 70%, send sub-agent and short-prompt traffic to the free
  pool and keep the paid pool for long, tool-heavy turns. Unknown quota never matches.
- **Visible.** `GET /v1/quota` shows the snapshots; `explain` shows the quota state a rule saw and
  whether it matched; decisions record which quota condition (if any) selected the route.
- **Advisory and fail-open.** Quota never refuses a request and never removes a target by itself; it
  only changes which rule matches. Where no rule uses quota, behavior is unchanged.
- **Measure before trusting it.** Because the endpoint is account-level and its semantics are not
  documented (what `status` values exist, what exactly a percent is a percent of), the first step
  ships the snapshot and the endpoint only, and the percent is compared with the cost humpyard
  measures from the ledger before any rule depends on it.
- **Out of scope:** per-model quota (the endpoint does not expose it; adapters may later), learning
  thresholds automatically, refusing requests on quota, and quota for providers that publish none.

## Capabilities

### New Capabilities
- `provider-quota`: poll a provider's published usage windows and expose the latest snapshot.

### Modified Capabilities
- `route-selectors`: a `quota` condition.

## Impact

New `src/quota.rs` (adapter, snapshot store, poller), config (`quota_source`, `quota_poll_secs`,
`quota_stale_secs`, the `when.quota` condition and its validation), `src/select.rs` (the condition),
`src/server` (the poller task, `/v1/quota`, `explain`, decisions). This is the first runtime network
use besides inference, so ADR 0015 bounds it. Docs: routing (a pacing recipe), configuration, api.
