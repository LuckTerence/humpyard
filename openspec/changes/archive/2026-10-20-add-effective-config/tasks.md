# Tasks

- [x] 1.1 `read_config` on keys: schema, validation, documented
- [x] 1.2 The configuration view: a pure function over the loaded `Config`, secrets omitted, sources labelled
- [x] 1.3 `GET /v1/config` with the 403 `config_not_readable` rule
- [x] 1.4 `humpyard_config_info{fingerprint}` metric, replaced on reload
- [x] 1.5 `check-config` prints the fingerprint
- [x] 1.6 `ctl config` with `--json` and `--fingerprint`
- [x] 1.7 Tests for each scenario, including that no secret appears; docs (`api.md`, `ctl.md`, `metrics.md`, `budgets.md` for the key setting); architecture test and `docs/architecture.md`; archive this change
