# Tasks

Each group stands alone and ships in its own change; this change records the decisions only.

- [ ] 1.1 ADR 0025, `docs/architecture.md` and `docs/roadmap.md` updated (done in this change's PR)
- [ ] 2.1 Effective-configuration endpoint with provenance, and the fingerprint as an endpoint and a metric
- [ ] 3.1 Override store (ledger table, expiry, audit rows), planning input, `explain`/decision marks, `overrides_active` metric and alert example
- [ ] 4.1 Read-only status page on the same API
- [ ] 5.1 Patch generation with a format-preserving TOML editor, validated by the real loader
- [ ] 5.2 Optional direct write (`[admin] allow_config_writes`), fingerprint-conditional and atomic
- [ ] 6.1 Architecture test: only the file writer and the override store can change effective configuration
