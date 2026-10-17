# Tasks

## 1. The loader (serve side, no network)
- [x] 1.1 `[discovery]` and provider `discover` in config with validation (needs `price_source`; ranges); tests
- [x] 1.2 `src/discovered.rs`: strict parse and merge of the generated file with every rule above; pools resolve over the merged targets; tests for each accept/reject case, including collision with a hand-written (provider, model) pair
- [x] 1.3 Reload fingerprint covers both files; only-the-generated-file-changed reloads; a bad file is ignored with a log and a gauge; `explain` marks generated targets; metrics; tests end to end

## 2. The engine (CLI side, network)
- [x] 2.1 `src/discover.rs`: read free chat models from the models.dev record (cost 0, text output, window), confirm against the provider's `/models` (key only to its own host); tests with fixtures and a mock provider that fails if a key reaches the dataset host
- [x] 2.2 `models check` with `--json` and exit codes; new, gone, unconfirmed, window drift; tests
- [x] 2.3 `models sync`: generated file, atomic write, unchanged-is-untouched, `.prev`, the keep/remove rule (add on both sources, keep on the provider, remove on the provider or on "now paid"; a dataset that omits a model for a run changes nothing), guardrails (`max_added_per_run`, `max_removed_fraction`, empty provider list, floor), `--every` loop that survives a failed run; tests including an outage and a poisoned dataset
- [x] 2.4 Architecture rules: the network module is used only by `main`, the pure loader by `config` and `server`; the serving-path test covers `discover`

## 3. Docs and decisions
- [x] 3.1 ADR 0022; ADR 0014 status corrected to accepted
- [x] 3.2 `docs/discovery.md` with the scheduling recipes, host schedulers first (cron, systemd timer, launchd, Task Scheduler running the one-shot command on the gateway's machine), then Docker (host cron plus a bind-mounted file), then the `--every` loop and a Kubernetes sidecar or CronJob; each recipe tested by running it once against a mock source; a section in the ModelRelay migration guide telling migrators they must schedule it; `configuration.md`, `routing.md`, `metrics.md`, `architecture.md`, `roadmap.md` (a new epic step: model discovery)
