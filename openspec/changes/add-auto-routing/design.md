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
- **Measure before building step 2.** The decision log is the prerequisite for tuning: without it
  there is no record of what the rules did over real traffic.

## Risks / Trade-offs

- A classifier adds latency and a small cost per request; mitigated by timeout, fallback and
  applying it only where rules leave a choice open.
- Per-request picks mean a conversation can switch pools mid-way; stickiness is a separate change.
- Misrouted private content is the worst failure: private rules must fail closed (private traffic
  falls back to another private pool or is refused, never to a public one) once that policy exists.
