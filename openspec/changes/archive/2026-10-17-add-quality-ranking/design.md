# Design

## Context

ModelRelay computes `qos = quality * availability * speed + speed` per model, from a quality score that is
Artificial Analysis's coding index when it knows the model and an estimate from metadata otherwise (on the
reference instance 23 of 53 usable models are measured, 30 estimated; measured scores run 0.52 to 0.76).
humpyard has the speed (observed latency, ADR 0020), health (breakers) and membership (pools, ADR 0021). It
has no quality number, and the data that would supply one (a benchmark index) is a third-party table that goes
stale, which ADR 0014 and 0022 say belongs to an operator's declaration or a reviewed suggestion, not to a
runtime fetch.

## Decisions

- **Quality is declared, not computed.** A `quality` on the target, 0 to 1, in the same spirit as
  `context_window`: a fact the operator states and can change in one line. Unknown is allowed and is treated
  honestly: it does not qualify for a quality floor, and it ranks as a neutral value rather than as the
  worst or the best. Where the number came from is the operator's business; the migration notes the source in
  a comment.
- **Two uses, because they answer different questions.** A pool's `min_quality` decides *which* targets a
  pool contains (load time, deterministic, explainable); `quality_weight` decides *in what order* a
  `fastest` route tries them (request time, from observed speed). Using only one would either hide weak models
  from a pool entirely or let them lead it.
- **A weight that defaults to today.** `quality_weight = 0` keeps the exact speed ordering, so no existing
  route changes. With a weight above 0 the score is a plain blend, `(1 - w) * speed + w * quality`, where
  `speed = min(1, latency_target_ms / observed_ms)`: at or under the target is full marks, slower falls off in
  proportion. It is deliberately simpler than ModelRelay's product-plus-speed so an operator can read a score
  and predict an order. Ties break by observed speed, then written order.
- **Measured targets only get a blended score.** A target with too few samples is still tried first to be
  measured (ADR 0020), so a strong model is not starved of its first requests and an unmeasured weak one is not
  trusted on a guess. Exploration and staleness are unchanged.
- **Estimated scores are not carried as facts.** ModelRelay marks which of its scores are estimated. A declared
  `quality` that is really a guess would make the floor and the ranking claim more than is known, so the
  migration declares only measured scores and leaves the rest as a comment; `--estimated-quality` is the
  explicit opt-in.
- **No runtime source, now.** Fetching a benchmark index would be a new third-party dependency on the
  serving path or a new reviewed-suggestion command. Both are possible later (a `models` suggestion in the
  style of `prices import`); this change only gives the number a place to live and a use.

## Risks / trade-offs

- A declared number can be wrong or stale; the cost is a slightly different order or pool, visible in
  `explain`, fixed in one line. Nothing is refused because of quality.
- With many unknown qualities the floor excludes them all; the empty-pool error names this, as it does for
  windows.
- The blend is a choice of formula. It is documented, tested, and the weight defaults to off.
