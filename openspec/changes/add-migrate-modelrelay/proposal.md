# Proposal

## Why

Steps 1 to 3 of "Replacing ModelRelay" give humpyard what a free pool needs: stated retry times, speed
ordering and pools by strength. What is left is getting there. A ModelRelay setup is a set of enabled
providers, their keys, and a catalog of free models with context windows and tags; rewriting that by hand as
humpyard providers, targets, strengths and pool routes is tedious and error-prone, and the reference
deployment has dozens of models behind two virtual models. Step 5, `migrate-modelrelay`.

## What Changes

- **A command, `humpyard migrate-modelrelay`**, that reads a running ModelRelay (its admin API) or two saved
  API responses, and **prints a humpyard configuration fragment**: providers, one target per usable model,
  and pool routes. It never edits a configuration file and never overwrites an existing output file.
- **Facts come from ModelRelay itself**, not from a bundled catalog: `/api/config` (which providers are
  enabled and have a key) and `/api/models` (each model's provider, id, context window, tags and whether it is
  banned). This is the user's own data from their own instance, so nothing has to be copied from another
  project and the bundled catalog (step 4) is not a prerequisite.
- **Keys are never read or copied.** ModelRelay's API reports whether a provider has a key; the output
  references `api_key_env = "GROQ_API_KEY"` (the variable name is derived from the provider) and a comment
  tells the operator to set it. The endpoint that returns key values is never called.
- **Drop-in route names.** Clients keep their model strings: the fragment defines routes named like
  ModelRelay's virtual models, `auto-fastest` (a `fastest` route over every generated target) and
  `tag:<strength>` (a `fastest` pool of that strength), each also with `+min_ctx:N` for every value passed
  with `--min-ctx`.
- **Honest about what does not carry over**: extra keys of a multi-account provider, banned or disabled
  models, ModelRelay's ping scores and quality ranking, and providers humpyard has no base URL for are listed
  in a report, not silently dropped.
- **Checked before it is shown**: the fragment is loaded through the real configuration parser (with
  placeholder keys), so a fragment that would not load is reported instead of printed as success.
- **Out of scope:** editing live configuration, migrating ModelRelay's request logs or telemetry, rotating
  several keys of one provider (humpyard has one key per provider), the bundled catalog (step 4), and
  retiring ModelRelay (step 6, the operator's).

## Capabilities

### Added Capabilities
- `cli`: the `migrate-modelrelay` command.

## Impact

`src/main.rs` (subcommand), a new `src/migrate.rs` (parsing the two responses and rendering the fragment),
a small table of base URLs for the providers ModelRelay supports, tests with recorded response fixtures.
Docs: a migration guide under `docs/`, `README`, `roadmap.md`.
