# route-selectors Specification

## Purpose
Defines how the gateway chooses the route for a request from facts about it (key, model, headers, tags, agent metadata), and that clients can narrow but never widen what a key may use.

## Requirements

### Requirement: Ordered selectors choose the route
The gateway SHALL evaluate `[[select]]` rules in order and route the request to the first matching rule whose route the key may use; when no rule applies it SHALL use the requested model as the route.

#### Scenario: Header selects a route
- **WHEN** a rule matches `header` `x-client` = `openclaw` and the request carries that header
- **THEN** the request is served by that rule's route and the response names the rule and route

#### Scenario: No rule matches
- **WHEN** no rule matches
- **THEN** the requested model is the route, and an unknown model is 404 as before

#### Scenario: First match wins
- **WHEN** two rules match
- **THEN** the earlier one decides

### Requirement: Conditions
A rule SHALL match only if every condition it gives holds: `model`, `key`, `profile`, `agent`, `task` (globs where `*` matches any text), `subagent`, `stream` (booleans), `header` and `tag` (names to globs). A rule without conditions matches every request.

#### Scenario: Tag condition
- **WHEN** a rule has `tag = { team = "infra" }` and the request has header `x-humpyard-tag-team: infra`
- **THEN** the rule matches

#### Scenario: Several conditions
- **WHEN** a rule requires a key glob and a subagent flag and only one holds
- **THEN** it does not match

### Requirement: Clients narrow, never widen
A selected route SHALL be one the key may use; a matching rule whose route the key may not use SHALL be skipped. Credential headers (`authorization`, `x-api-key`, `cookie`, `proxy-authorization`) SHALL never be matchable.

#### Scenario: Header cannot escalate a restricted key
- **WHEN** a key limited to route A sends a header that matches a rule for route B
- **THEN** the rule is skipped and the request follows the next applicable rule or the requested model

### Requirement: Selector config is validated
Loading SHALL fail naming the problem when a rule's route does not exist, a condition is unknown, a matched header is a credential header, rule names repeat, or a rule follows a catch-all rule.

#### Scenario: Unknown route
- **WHEN** a rule names a route that is not configured
- **THEN** config loading fails naming the rule and the route

### Requirement: Explain a routing decision without sending the request
The gateway SHALL answer `POST /v1/route/explain`, for the authenticated key, with the selected route and rule, the outcome each rule had (matched, the reasons it did not, and whether the key may use its route), the budget state, the targets and endpoint health the route would use, the route's fallback classes, and an outcome name. It SHALL use the same selection, budget and policy logic as real requests, SHALL NOT call any provider, and SHALL NOT write the ledger.

#### Scenario: Chosen rule and reasons
- **WHEN** a client asks to explain a request with a matching profile header
- **THEN** the report names the chosen rule and route and lists every other rule with why it did not match

#### Scenario: Skipped because of the key
- **WHEN** a rule matches but the key may not use its route
- **THEN** the report shows the rule as matched with `key_may_use_route` false, and the selected route comes from the next applicable rule or the requested model

#### Scenario: Refusals are named
- **WHEN** the request would be refused
- **THEN** the outcome is one of `forbidden`, `unknown_model`, `budget_exhausted` or `no_eligible_target`, with a message

#### Scenario: Agreement with real requests
- **WHEN** the same key, model and headers are sent as a real request
- **THEN** the real response's route and rule equal the explained ones

#### Scenario: Authentication required
- **WHEN** keys are configured and the call has none
- **THEN** it is refused with 401

### Requirement: Content conditions
A rule MAY require `prompt_tokens` within inclusive `min` and `max` bounds of the estimated prompt size, `tools` to be true or false according to whether the request defines tools, and `images` according to whether it carries images. The prompt-size estimate SHALL be computed only when a rule needs it, and an unknown size SHALL NOT match a `prompt_tokens` rule.

#### Scenario: Long prompts
- **WHEN** a rule requires `prompt_tokens` of at least 2000 and the request's estimate is 2500
- **THEN** the rule matches

#### Scenario: Bounds are inclusive
- **WHEN** the estimate equals `min` or `max`
- **THEN** the rule matches

#### Scenario: Tools and images
- **WHEN** a rule requires `tools = true` and the request defines tools, or `images = true` and a message carries an image
- **THEN** the rule matches

### Requirement: Content conditions are explainable and validated
`explain` SHALL accept `prompt_tokens`, `tools` and `images` and report which content condition stopped a rule; configuration SHALL be rejected when a `prompt_tokens` range has neither bound or a `min` above its `max`.

#### Scenario: Reason shown
- **WHEN** explain is asked with `prompt_tokens` 10 against a rule requiring at least 2000
- **THEN** the rule's mismatch reads "prompt_tokens: wanted at least 2000, got 10"

#### Scenario: Invalid range
- **WHEN** `prompt_tokens = { min = 10, max = 5 }`
- **THEN** loading fails saying min is above max

### Requirement: Fastest route
The gateway SHALL support a route type `fastest` with `targets`, optional `explore` (a probability from 0 to 1, default 0.1), `min_samples` (default 3), `max_age_secs` (default 600), `seed`, `fallback_on` and `allowed_classes`. For each request it SHALL order the targets that remain eligible after class, policy, capability and health subtraction by their observed time to answer, fastest first, and attempt them in that order with the route's fallback.

#### Scenario: The faster target is tried first
- **WHEN** a `fastest` route has targets `a` and `b`, both measured, and `b` answers faster
- **THEN** requests try `b` first and `a` only if `b` fails over

#### Scenario: Unmeasured targets are tried early
- **WHEN** a target has fewer than `min_samples` answered attempts, or its figure is older than `max_age_secs`
- **THEN** it is ordered before measured targets, in the route's written order

#### Scenario: Exploration
- **WHEN** `explore` is 0.1 and the seeded draw selects exploration for a request
- **THEN** a random eligible target is tried first and the rest follow in speed order, and `explore = 0` never explores

#### Scenario: Ordering never adds a target
- **WHEN** the fastest target is excluded by class, policy, capability or health
- **THEN** it is not tried, however fast its figure

### Requirement: What counts as a sample
The gateway SHALL update a target's observed time only from answered attempts and SHALL keep it in memory as a smoothed figure with a sample count. Failed attempts SHALL NOT change it.

#### Scenario: A failure is not a fast sample
- **WHEN** an attempt on a target times out or returns a server error
- **THEN** the target's figure and sample count are unchanged

### Requirement: Order is visible
The decision log SHALL record the order a `fastest` route used and whether the first target was chosen as fastest or by exploration, and route explain and statistics SHALL show each target's current figure and sample count.

#### Scenario: Explain shows the figures
- **WHEN** a client explains a `fastest` route
- **THEN** each target lists its smoothed time and sample count, and the order a request would use now

### Requirement: Pools are explainable
Route explain SHALL report, for a pool route, its strength, its `min_context_window` when set, its members, and each target that carries the strength but was left out with the reason (`context_window_below` or `context_window_undeclared`).

#### Scenario: Explain shows why a target is missing
- **WHEN** a client explains a pool route and a target with the strength was left out for an undeclared window
- **THEN** the report lists that target with the reason `context_window_undeclared`

### Requirement: Quality-weighted ordering
A `fastest` route SHALL accept `quality_weight` from 0 to 1 (default 0) and `latency_target_ms` (default 2000, at least 1). With a weight of 0 the order SHALL be exactly the speed ordering. With a weight `w` above 0, each target with a trustworthy speed figure SHALL be ranked by `score = (1 - w) * speed + w * quality`, where `speed = min(1, latency_target_ms / observed_ms)` and `quality` is the declared quality, or the route's `unknown_quality` (default 0.5) when none is declared; ties SHALL break by observed speed, then written order. Targets with too few samples or a stale figure SHALL still be tried first, and exploration SHALL be unchanged.

#### Scenario: Weight zero is today's behavior
- **WHEN** a route has no `quality_weight`
- **THEN** its order is the speed ordering, whatever the targets' qualities

#### Scenario: Quality outweighs a small speed gap
- **WHEN** weight is 0.7 and a target with quality 0.8 answers in 600 ms while one with quality 0.4 answers in 300 ms (target 2000 ms)
- **THEN** the first is tried first

#### Scenario: A very slow strong model does not lead
- **WHEN** a quality 0.9 target answers in 20000 ms and a quality 0.6 target in 800 ms, with weight 0.5
- **THEN** the faster, weaker target is tried first

#### Scenario: Explain shows the parts
- **WHEN** a client explains a route with a weight
- **THEN** each target shows its quality, speed score and score
