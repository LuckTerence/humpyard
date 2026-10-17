# 0022. Free-pool membership may be updated on a schedule without review

- Status: accepted
- Date: 2026-10-17
- Supersedes: none (refines [0014](0014-prices-are-proposed-never-fetched-at-runtime.md))
- Change: `openspec/changes/add-model-discovery`

## Context

Free providers add and remove models constantly, and a free pool written once goes stale. ModelRelay kept its
pools current by itself. ADR 0014 keeps third-party data out of the serving process and has people review
every change to prices, because a silent wrong price corrupts budgets. Membership of a free pool fails
differently: a wrong member costs a slightly worse pick or a failed attempt that health skipping absorbs, not
a wrong bill, provided no paid model can get in.

## Decision

Discovery is two commands and a file. `humpyard models check` reports; `humpyard models sync` rebuilds a
generated file of `[targets.auto-*]` and may be scheduled; `serve` only reads that file and never fetches
anything. The update applies without review, under conditions that bound it: it is **off** until switched on
at two levels (a global `[discovery] enabled`, and `discover` on each provider, which can be turned off again
with `enabled = false`); it only ever adds **free chat models**, and only when **two sources agree** (the
models.dev dataset and the provider's own `/models`); a model is kept while the provider lists it and the
dataset has not recorded a cost, and removed when the provider drops it or it turns paid, so a lagging dataset
removes nothing; a run is **refused** when it would add more than `max_added_per_run` or remove more than
`max_removed_fraction` of a non-trivial set, or when the new file would not load; the file is applied by a
**strict loader** that accepts it whole or ignores it whole, forces the configured class (so a generated
target can never reach a strict private route), requires zero prices, forbids duplicating a hand-written model
and can never make a configuration fail to load; and nothing is ever added for a paid provider (`check --paid`
only reports). The provider key goes only to the provider's own host and never to the dataset. Prices, keys,
routes and everything written by hand remain proposed and reviewed under ADR 0014.

Alternatives considered: discovery inside `serve` on a timer (routing would depend on a third party's data
and timing, and a parser for untrusted data would sit in the process that answers every client; it may come
later as an opt-in built on the same module); a generated merge request for review (right for prices, but it
defeats a pool that tends itself, and the conditions above are what make skipping review acceptable); one
source only (either one alone can be wrong, and agreement bounds the damage to models the provider really
offers to this key and the dataset says cost nothing); removing a model as soon as either source drops it
(the thinner, slower dataset is the likely cause of flapping); a grace period before removing (state to keep,
and the provider's own list is the authority on availability anyway).

## Consequences

A free pool follows the providers without anyone editing it, and the cost is a small set of bounded,
visible changes: generated targets are named `auto-*`, marked in `explain`, counted by metrics, and listed by
every run. The operator owns the schedule (the gateway runs nothing on its own), and the documentation gives
recipes for cron, systemd, launchd, Windows, Docker and Kubernetes. New models carry only the strengths the
operator set for the provider, so they reach `general`-style pools and not curated ones until labelled by
hand. "Free" means zero per token, not unlimited, so rate limits still apply.
