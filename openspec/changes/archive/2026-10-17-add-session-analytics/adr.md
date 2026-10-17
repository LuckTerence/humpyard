# ADR Review

## Reviewed

ADR 0013 (decisions are metadata only, statistics are computed on read) governs this: no content is read,
nothing new is stored beyond an index, and the per-key read model is unchanged. ADR 0006 (money is integer
micro-USD) governs the estimate. ADR 0014 (prices come from configuration only) is why the dollar estimate
is computed in the server from the loaded config, never stored. ADR 0016 (metric labels from configuration
or fixed sets) is why session ids stay out of Prometheus.

## Outcome

No decision needs superseding and no new durable commitment is made: this is read-only measurement, and
the stickiness it informs would get its own ADR if built.

## New ADRs

None.
