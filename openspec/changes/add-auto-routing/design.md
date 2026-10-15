# Design

## Context

Selectors (ADR 0011) already evaluate ordered rules over key, model, headers, tags, agent
metadata, prompt size, tools and images, first match wins, and clients can narrow but never widen.
`llm_classifier` routes already exist as a Switchyard-backed route type with a judge target.

## Decisions

- **Phase it by risk.** Step 1 is pure configuration and documentation, usable immediately and
  testable with `explain`. Step 2 adds classification only if the rules prove too coarse. Step 3
  (learning from outcomes) is explicitly not planned.
- **Rules first, classifier as a rule action.** Deterministic rules stay the primary mechanism
  (cheap, explainable, no extra latency). A `classify` action is used for the cases rules cannot
  express, and always has a fallback pool and a timeout.
- **Safety stays as is.** The selected route must still be one the key may use, so an auto route
  cannot widen access; the budget and routing policy apply to the chosen route.
- **Intent through headers, not model names.** Clients that cannot vary headers per request get one
  provider entry per intent, each with a static `x-humpyard-profile`; this works today.
- **Statistics before cost-based choice.** Like a database optimizer, routing should earn
  statistics before acting on them: `add-routing-statistics` summarizes the decision log
  (failure mix, latency percentiles, cost per request) read-only. Cost-aware ordering inside the
  set a rule allows, with exploration and operator overrides, comes after that and needs its own
  proposal; rules for access and privacy stay hard constraints a cost model cannot override.
- **Strictness is opt-in, per route.** Some deployments want a private route to degrade to a public
  one when the private model is down; a global fail-closed rule would break them. The class labels
  and the allowed-class list make the policy explicit either way, and the flag on decisions makes
  today's silent behavior observable first.
- **Strict fallback is an eligibility rule, applied at planning (ADR 0017).** A route may set
  `allowed_classes = ["private"]`. Targets of any other class (and unclassed targets) are removed
  from the route's target list in `plan_route`, in the same step where the routing policy narrows
  targets (ADR 0008), so the algorithm, the failover and the tier substitution only ever see allowed
  targets. A disallowed target is not "refused after a failure": it is never constructed for the
  request, so no failure path, retry or substitution can reach it. This is simpler and safer than
  checking at the moment of fallback, which would depend on Switchyard's internals.
  - *Config is validated fail-closed.* A class in `allowed_classes` must be a valid label; every
    target the route mentions must have a class (an unclassed target on a strict route is an error,
    not a silent exclusion); and at least one target must be allowed, so a route cannot be
    configured to always refuse. Routes without `allowed_classes` behave exactly as today.
  - *What it does not override.* Health still never denies service (ADR 0009): a strict route whose
    only allowed target is cooling down still tries it, and returns that target's error. Budgets
    still narrow within the allowed set. Selectors still choose the route; a client can narrow but
    never widen (ADR 0011), and no header can add a class.
  - *When the allowed targets fail*, the client gets the upstream error (it is not rewritten), and
    the failure is visible three ways: the decision lists `excluded_targets` (the targets the class
    list removed), `explain` marks each target `allowed` or `excluded` with the reason, and the
    `humpyard_fallback_blocked_total{route}` metric counts requests that failed on a strict route
    that had excluded targets, which is exactly the "it would have leaked to public" signal.
  - *Decisions gain a field.* `excluded_targets` is stored (ledger schema v4, a JSON column like
    `eligible_targets`) so a refusal is auditable after the fact. It holds target names from
    configuration only.
- **Stickiness depends on the client.** Switchyard reads session ids from Claude Code, Codex and
  `x-switchyard-session-id`. A client that sends none gets no stickiness, and a static provider
  header cannot carry a per-conversation id, so the investigation measures coverage per client
  before promising it.
- **`classify` has a measurable trigger.** Build it only if the decision log shows a high share of
  requests landing on the catch-all, or the default pool's cost per request is above a threshold
  agreed in advance; otherwise it stays deferred.
- **Measure before building step 2.** The decision log is the prerequisite for tuning: without it
  there is no record of what the rules did over real traffic.

## Risks / Trade-offs

- A classifier adds latency and a small cost per request; mitigated by timeout, fallback and
  applying it only where rules leave a choice open.
- Per-request picks mean a conversation can switch pools mid-way; stickiness is a separate change.
- Misrouted private content is the worst failure: private rules must fail closed (private traffic
  falls back to another private pool or is refused, never to a public one) once that policy exists.
