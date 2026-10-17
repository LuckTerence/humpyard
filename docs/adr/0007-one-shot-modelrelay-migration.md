# 0007. One-shot modelrelay migration, not a runtime importer

- Status: accepted
- Date: 2026-10-07 (recorded retroactively; decided 2026-10-06)
- Supersedes: none
- Change: [add-core-gateway](../../openspec/changes/archive/2026-10-06-add-core-gateway) (decision recorded in [background.md](../background.md#modelrelay-migration-decided-2026-10-06))

## Context

This gateway succeeds the Node.js modelrelay fork, which stays as the long-term-support branch. We
first planned to read `~/.modelrelay.json` directly. Inspecting the real file and the fork's
documentation showed it holds only credentials, toggles, custom endpoints, bans, tags and pinning;
the built-in provider and model catalog lives in code (`sources.js`, `tags.js`, `scores.js`) and
keys can also come from about fifteen environment variables. The fork changes its own schema and
stores keys in plaintext.

## Decision

No runtime reader of the legacy file. A re-runnable `migrate-modelrelay` command will read the JSON
and the environment variables and write our TOML: keys become `api_key_env` references with a
printed list of variables to set, and the built-in catalog is ported once as bundled data. Custom
endpoints, bans, tags and pinning carry over.

## Consequences

- Our schema is not tied to a moving external format, and plaintext keys are never copied
  ([0004](0004-no-plaintext-secrets.md)).
- Users run one command to switch; the fork can keep evolving independently.
- Not yet built: the command and the bundled catalog are planned future work.

## Amended 2026-10-16: read the admin API, not the file, and no bundled catalog

The decision above assumed the file plus a catalog ported once as bundled data. Building the command
showed a better source. The file holds only credentials, toggles, custom endpoints, bans and tags, so
reading it alone cannot say which models exist; and a port of the catalog is a table of another project's
data that goes stale. ModelRelay's admin API already serves exactly what a migration needs, from the
instance the operator actually runs: `/api/config` (which providers are enabled, whether each has a key, and
custom endpoints) and `/api/models` (each model with its provider, context window, tags, and whether it is
banned), and its keys are masked there. So `humpyard migrate-modelrelay` reads those two responses (live, or
saved to files for offline use), never calls the endpoint that returns key values, and needs no bundled
catalog. The rest of the decision stands: no runtime reader, a re-runnable command, keys become
`api_key_env` references, and plaintext keys are never copied. The risk this trades for the one above is
tying the command to the shape of two API responses; it ignores unknown fields, reports a response it cannot
read, and the guide names the ModelRelay version it was checked against (1.28). A bundled free-model catalog
(step 4 of the roadmap) remains possible later for operators who do not run ModelRelay.

