# Design

## Context

ModelRelay (`@schaetzkc/modelrelay`) keeps `~/.modelrelay.json` with `apiKeys` (a string, or an array for
several accounts) and per-provider `enabled`, plus `openai-compatible:<id>` instances that carry their own base
URL. Its catalog (`sources.js`, `tags.js`) lists models per provider with a context window and tags drawn
from `coding`, `reasoning`, `general`, `fast`, `agentic`: the same vocabulary humpyard strengths were designed
around. Its admin API serves `/api/config` (providers, enabled, whether a key is present) and `/api/models`
(live model rows with provider, id, context, tags, banned state). A separate endpoint returns a key value.

## Decisions

- **Read the API, not the file.** `~/.modelrelay.json` holds plaintext keys and is readable only on the
  machine running ModelRelay; the API gives the same facts without key values, from anywhere that can reach
  it, and sees the models as ModelRelay actually has them (including tag edits and bans made in its UI).
  `GET /api/config/keys/...` (key values) is never called. The admin token comes from an environment
  variable named on the command line, never an argument.
- **Offline mode.** `--config FILE --models FILE` take saved responses, so the command is testable, usable
  where the admin API is not reachable, and never needs a token or network at all.
- **A fragment, printed.** The output is a TOML fragment on stdout (or `--out FILE`, refused if it exists).
  It is meant to be reviewed and merged by the operator, as `prices import` proposals are (ADR 0014): public
  or third-party data is only ever a suggestion.
- **Targets: one per usable model.** Named `<provider>-<model>` reduced to lowercase letters, digits and `-`;
  collisions get a numeric suffix. `class = "public"` by default (`--class` overrides) because ModelRelay's
  catalog is free hosted models. Strengths are ModelRelay's tags that are in the shared vocabulary; unknown
  tags are reported. `context_window` is the model's reported window when present.
- **Drop-in route names.** Routes named `auto-fastest`, `tag:<strength>` and `...+min_ctx:N` are legal TOML
  quoted keys and legal route names, so clients and key `allowed_routes` need no change. `auto-fastest` is a
  `fastest` route over all generated targets; `tag:X` is a `fastest` pool of strength `X`. A `+min_ctx:N`
  variant is a pool with `min_context_window = N` (and, for `auto-fastest`, a pool over a catch-all strength
  `all` that the command adds to every generated target).
- **Several keys of one provider.** humpyard has one key per provider, and a provider's account rotation is
  not part of this epic. The first key's provider is migrated; the report says how many extra keys exist so
  the operator can add providers by hand or ask for rotation.
- **Unknown providers.** A provider with no known OpenAI-compatible base URL is listed in the report with the
  reason; `openai-compatible:<id>` instances use their own base URL. Nothing is guessed.
- **Self-check.** The fragment plus a minimal header is parsed by the real configuration loader with
  placeholder keys. A failure prints the error and exits non-zero without presenting the fragment as ready.

## Risks / trade-offs

- A free model that ModelRelay lists but whose provider requires a different path than `/v1/chat/completions`
  would load and fail at run time; the base-URL table is limited to providers known to be OpenAI-compatible.
- Tags are ModelRelay's judgments; they are carried over as strengths, which only decide pool membership
  (ADR 0021), so a questionable tag costs a pool's composition, never a refused request.
- Context windows in `/api/models` are what ModelRelay believes (it can cap them by observed quota); they are
  copied as declarations, which humpyard enforces only on certainty, so a window that is too small removes a
  target only from requests that clearly exceed it.
