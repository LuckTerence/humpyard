# Spec Delta

## ADDED Requirements

### Requirement: models check
The CLI SHALL provide `humpyard models check --config FILE [--provider NAME] [--json]` that, for each provider with `discover` set, fetches its models.dev source and (unless `confirm = false`) the provider's own `/models`, and reports models that both list as free chat models and that no target uses, configured endpoints the source no longer lists, and declared context windows that differ. It SHALL NOT change any file, and SHALL exit 0 when nothing is to review, 1 when something is, and 2 when a source or the config could not be used.

#### Scenario: A new free model
- **WHEN** both sources list a free chat model that no target uses
- **THEN** the report names it with its window and the exit code is 1

#### Scenario: A model only one source lists
- **WHEN** models.dev lists a free model that the provider's `/models` does not
- **THEN** it is reported as unconfirmed and is not proposed

### Requirement: models sync
The CLI SHALL provide `humpyard models sync --config FILE [--every DURATION]` that rebuilds the generated file named by `[discovery] file` from the models both sources confirm, writing it atomically only when its content changes and keeping the previous version as `.prev`. It SHALL NOT edit the main configuration, SHALL refuse a run that adds more than `max_added_per_run` targets or removes more than `max_removed_fraction` of a non-trivial generated set (writing nothing and exiting 1), and SHALL skip and report a provider whose source returns no models. With `--every` it SHALL repeat, surviving a failed run.

#### Scenario: Idempotent
- **WHEN** a run finds nothing changed
- **THEN** the file is not rewritten

#### Scenario: Adding needs both sources, keeping needs the provider
- **WHEN** a model is new and only one source lists it as free
- **THEN** it is not added
- **WHEN** a model already in the generated file is still listed by the provider but models.dev has dropped or lost it
- **THEN** it is kept

#### Scenario: Removal
- **WHEN** the provider no longer lists a generated model, or models.dev now records a nonzero cost for it
- **THEN** it is removed at the next run

#### Scenario: A flapping dataset
- **WHEN** models.dev omits a generated model for one run and lists it again the next, and the provider lists it throughout
- **THEN** the file is unchanged for both runs and the gateway does not reload

#### Scenario: A source outage
- **WHEN** a source returns an empty list for a provider that has generated targets
- **THEN** nothing is written, the guardrail is reported, and the existing file keeps serving

#### Scenario: Keys
- **WHEN** the command confirms models with a provider
- **THEN** the provider's key is sent only to that provider's own host and none is sent to models.dev

### Requirement: Scheduling is documented
The documentation SHALL state that the gateway does not run discovery itself, and SHALL give recipes for running `models sync` on a schedule, including for migrators from ModelRelay, which refreshed itself every 30 minutes.

#### Scenario: A migrator reads the guide
- **WHEN** a user follows the ModelRelay migration guide
- **THEN** it tells them to schedule `models sync` and links the recipes

#### Scenario: No Kubernetes needed
- **WHEN** a user runs the gateway on one machine or in one container
- **THEN** the guide's first recipe is a cron or other host scheduler running the one-shot command against the file the gateway reads, with a copy-pasteable crontab line, a systemd timer pair, a launchd plist and a Task Scheduler line
