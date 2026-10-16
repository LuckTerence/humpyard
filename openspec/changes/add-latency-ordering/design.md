# Design

## Context

A route's targets are tried in order by the algorithm. Passthrough tries them as written; `random` picks by
weight. Both sit behind subtract-only eligibility (class and policy, capabilities, health). The attempt
timing the decision log records (`ms` per attempt) is the only latency signal humpyard produces, and it is
measured on live traffic.

## Decisions

- **A route type, not a flag on passthrough.** "Order by speed" changes what a route means, and a separate
  type keeps `passthrough` exactly as deterministic as the docs say. `fastest` reads like `passthrough` in
  config and can later share an ordering core with cost-aware ordering.
- **Time to answer, not time to first token.** The figure is the full time of an answered attempt. Streaming
  time to first byte would favour fast starters that then crawl; the full time ranks what a client waits for.
  For streams the attempt time already ends when the stream ends. Time per output token is a later
  refinement and would need per-request normalisation.
- **A smoothed figure per target, in memory.** An exponentially weighted moving average (weight 0.3 on the
  newest sample) keyed by target name, with a sample count. In memory because it is cheap on the request path
  (no ledger query), needs no schema, and a restart only costs a few exploratory requests. Rebuilding it from
  the ledger at startup is possible later and not needed to be correct.
- **Unmeasured targets first.** A target with fewer than `min_samples` (default 3) answered attempts sorts
  before measured ones, in the route's written order, so a new target is measured before it is judged. It also
  means a freshly started gateway behaves like a passthrough route until it has data.
- **Exploration is per request and seeded in tests.** With probability `explore` the first try is a random
  eligible target and the rest follow in speed order. `seed` makes it reproducible, as `random` routes do.
  `explore = 0` is pure speed ordering.
- **Only answered attempts count.** A timeout counted as a slow sample would make a dying target look merely
  slow; a failure counted as fast would be worse. Failures stay health's business, which already removes a
  failing target; the figure of a skipped target simply stops updating, and when its cooldown ends it is
  probed like any other request and measured again.
- **Staleness.** A figure older than `max_age_secs` (default 600) counts as unmeasured, so a target that was
  slow ten minutes ago, then idle, gets another look.
- **Order is computed at plan time from eligible targets only**, after class, policy, capability and health
  have removed theirs, so ordering never reintroduces a removed target and explain can show the order the
  request would use.

## Risks / trade-offs

- With few requests per target the figure is noisy. Counts are shown next to figures, and exploration plus
  `min_samples` bound how long a bad early sample can mislead.
- Fast but wrong: speed says nothing about quality. Pools should hold targets of comparable capability;
  strengths (step 3) are how an operator says which are comparable.
- A shared figure per target mixes request sizes: a target that only gets large prompts looks slower. That is
  accepted for a first version; the figure is documented as a rough ranking and per-size buckets are future
  work.
