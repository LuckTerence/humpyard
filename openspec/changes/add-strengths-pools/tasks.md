# Tasks

## 1. Config
- [x] 1.1 `strengths` on targets and `pool` on `passthrough`, `fastest` and `random` routes: schema, validation (label rules, exclusivity with `targets` and `weights`), tests for each scenario
- [x] 1.2 Resolution at load: membership by strength and declared window, sorted by target name, written into the route's targets; empty pool error naming the strengths that exist; tests including a multi-endpoint target and an undeclared window
- [x] 1.3 Reload: a changed membership rebuilds the route and is logged; an unchanged one reuses it; tests

## 2. Visibility
- [x] 2.1 `route/explain` pool report (strength, window, members, left-out with reasons); end to end through the gateway with a pool served by a `fastest` route
- [x] 2.2 Strict-route interaction: a pool on a route with `allowed_classes` is narrowed per request, never widened; test

## 3. Docs and decisions
- [x] 3.1 ADR 0021; `routing.md` (Pools and strengths), `configuration.md`, `architecture.md`; the roadmap marks step 3 built
