# Design

## The view

`GET /v1/config` returns JSON built from the loaded `Config` at the moment of the request, so it is exactly what the
gateway is serving. Sections:

- `config`: `fingerprint`, `loaded_at_ms`.
- `settings`: listen address, reload and retention settings, health and budget settings, whether a ledger is set.
- `providers`: name, `base_url`, whether an API key is set (never the value), timeouts and retries, header
  names (values omitted: provider headers can carry session identifiers).
- `targets`: name, `class`, `strengths`, `quality`, `source` (`file` or `generated`), and its endpoints with provider,
  model, price, `context_window`, `tools`, `vision`.
- `routes`: name, type, `source` (`file`), targets or, for a pool, the resolved members and the filters that produced
  them, `allowed_classes`, `fallback_on`, and the guarded tail with its guard.
- `selectors`: the rules in order.
- `keys`: ids, their `allowed_routes`, limits and `over_budget`, and `read_config`; never a hash.

The `source` of a target is `generated` when its name is in `Config.generated_targets`, otherwise `file`. The `override`
source belongs to the override store (task 3.1) and does not exist yet; the field's vocabulary leaves room for it.

## Who may read it

Provider addresses and prices are operator information. A key may read the view only if it sets `read_config = true`;
any other key gets 403 `config_not_readable`. A gateway with no `[keys]` is already open and unbudgeted, so the view is
open too. This is a deliberately small step toward ADR 0025's profiles: when profiles exist, `read_config` becomes a
profile capability.

## The fingerprint

It is the value the gateway already computes when it loads the configuration (the hash of the file text and the
generated file), already shown in `/v1/health`. Three places show it so drift can be detected:

- `/v1/config` and `/v1/health` (the same value),
- the metric `humpyard_config_info{fingerprint="…"} 1`; on a reload the old series is removed so the label does not
  accumulate,
- `humpyard check-config FILE` prints `fingerprint: …`, computed by the same function over the same files.

An operator compares the running value with the one `check-config` prints for the rendered configuration.

## ctl

`humpyard ctl config` prints the view as indented text with each target's source; `--fingerprint` prints only the
fingerprint (for scripts and drift checks); `--json` prints the body unchanged. A 403 is a refusal (exit 1) like any
other.
