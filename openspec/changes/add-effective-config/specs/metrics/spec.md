# Spec Delta

## ADDED Requirements

### Requirement: The configuration fingerprint is a metric
The gateway SHALL export `humpyard_config_info` with the label `fingerprint` set to the fingerprint of the loaded configuration and the value 1. After a reload it SHALL export only the new fingerprint, so one series exists at a time.

#### Scenario: A reload replaces the series
- **WHEN** the configuration is reloaded and its fingerprint changes
- **THEN** the metric shows the new fingerprint and no series for the old one
