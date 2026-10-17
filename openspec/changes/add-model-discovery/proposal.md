# Proposal

## Why

ModelRelay keeps its free pools current by itself: every 30 minutes it asks each provider which models exist
and adds the new ones and drops the gone ones. A humpyard pool written once, or migrated once
(`migrate-modelrelay`), is a snapshot: a new free model never joins, a removed one lingers until health
skipping routes around it. The goal of the "Replacing ModelRelay" epic is free pools that need no tending, so
discovery is the missing part, not an extra.

## What Changes

- **Providers opt in** to discovery: `discover = { strengths = ["general"] }` on a provider (which already
  names its models.dev provider through `price_source`).
- **`humpyard models check`** reports what discovery would change: models the source lists that no target uses
  (free chat models only), configured models the source no longer lists, and declared windows that drifted.
  Exit codes like `prices check` (0 clean, 1 something to review, 2 unusable), so a scheduled job can alert.
- **`humpyard models sync`** writes a generated file, `discovered.toml`, containing only `[targets.auto-*]`:
  one target per free chat model that **both** models.dev (free, chat, context window) and the provider's own
  `/models` (available to your key) confirm. It is rewritten atomically, only when it changes, and refused
  when a guardrail trips. `--every 6h` makes it a loop for a sidecar or a second container.
- **`serve` reads the file, never fetches.** `[discovery] file = "..."` names the generated file. At load and
  on every reload it is parsed strictly and merged into the targets, so pools (by strength) and `auto-fastest`
  pick the new targets up with the existing hot reload. A missing, empty or invalid file is ignored and logged:
  the gateway serves with the main config alone.
- **Guardrails**, because this applies without review: opt-in per provider; free chat models only; two sources
  must agree; at most `max_added_per_run` additions; a run that would remove more than
  `max_removed_fraction` of the generated targets is refused (an outage returning an empty list); hand-written
  targets are never touched or duplicated; an `exclude` list; generated targets are visibly `auto-` and
  reported as such in `explain` and the decision log; an empty `discover` list or deleting the file is the kill
  switch.
- **Scheduling is the operator's job, and documented as such.** The gateway does not run discovery for you; a
  migration guide section and a discovery guide explain how to run `models sync` on a schedule. The plain
  case comes first: a one-shot `models sync` from **cron, a systemd timer, launchd or Task Scheduler on the
  machine that runs the gateway**, writing the file the gateway reads, with nothing else to set up. Then a
  host cron job with a bind-mounted file for Docker, and the loop (`--every`) or a Kubernetes sidecar for
  clusters. Kubernetes is one deployment among several, not the assumed one.
- **ADR 0022** records the one deliberate departure from ADR 0014: a scheduled, applied-without-review update,
  limited to the membership of explicitly opted-in free pools, with those guardrails. Prices and everything
  else stay proposed and reviewed. ADR 0014's status is corrected to accepted. Running discovery inside
  `serve` as an opt-in mode is the intended later step and is out of scope here; the engine is written so it
  is a thin wrapper.
- **Out of scope:** discovery inside `serve`, inferring `coding`, `fast` or `agentic` (providers do not
  report them; operators set default `strengths`, or label by hand), paid models, a bundled catalog, and
  changing prices.

## Capabilities

### Added Capabilities
- `cli`: `models check` and `models sync`.
### Modified Capabilities
- `gateway-config`: `discover` on providers, the `[discovery]` section, the generated file.

## Impact

New `src/discover.rs` (network: sources, confirmation, check, sync, guardrails; used only by `main`) and
`src/discovered.rs` (pure: strict parse and merge of the generated file; used by `config` and `server`);
`src/config` (`discover`, `[discovery]`), `src/server.rs` (fingerprint and reload of the file), `src/metrics.rs`
(file state, count). Docs: `docs/discovery.md` (new, with scheduling recipes), `docs/migrating-from-modelrelay.md`,
`configuration.md`, `routing.md`, `metrics.md`, `roadmap.md`, `architecture.md`; ADR 0022; ADR 0014 status.
