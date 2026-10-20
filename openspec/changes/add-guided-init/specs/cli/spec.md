# Spec Delta

## ADDED Requirements

### Requirement: init command
The CLI SHALL provide `humpyard init`, which writes a new configuration file chosen interactively on a terminal or from flags otherwise. It SHALL read each provider key only from the environment variable the user names, SHALL NOT print, log or write a key value, and SHALL NOT overwrite an existing output file.

#### Scenario: Existing output file
- **WHEN** the output path already exists
- **THEN** the command fails without writing

#### Scenario: A key is never revealed
- **WHEN** a provider's variable is set
- **THEN** the command reports only that it is present and no output or file contains its value

#### Scenario: A missing key
- **WHEN** a provider's variable is unset
- **THEN** the provider is written without live checks and the command says which variable to set

### Requirement: Models are checked live
The command SHALL write a model only when the provider lists it and it answers one small request. A model that is not listed or does not answer SHALL be left out and reported with the reason.

#### Scenario: A retired model
- **WHEN** a preferred model is no longer listed by its provider
- **THEN** it is not written and the command picks another listed model or reports that none is available

### Requirement: The written file loads and is proven
The command SHALL load the file with the configuration parser before writing it and SHALL NOT write a file that does not load. It SHALL write a loopback listener by default, send one request through the gateway in process, and print the reply, the target that served it and the time it took.

#### Scenario: A working setup
- **WHEN** a provider key is set and a model answers
- **THEN** the file is written, a test request succeeds, and the command prints the serve command and a client snippet

### Requirement: Non-loopback needs a key
A non-loopback listen address SHALL be refused unless the command also creates a virtual key with a budget, which it SHALL print once.

#### Scenario: Exposing the gateway
- **WHEN** `--listen` is a non-loopback address
- **THEN** a key with a budget is created and printed once, and the file includes its hash
